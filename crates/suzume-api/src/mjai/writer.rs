//! Streaming projection of successfully applied core operations into mjai events.
//! Only the bounded current-round state is retained; history is not cloned.
use super::{Event, Pai};
use crate::config::{AppConfig, EffectiveAiConfig};
use crate::decision::DecisionView;
use crate::logging::{error, seat, validate_decision, validate_reason, EndReason, LogLimits};
use crate::types::{ApiError, WireAction};
use serde_json::{json, Value};
use std::io::Write;
use suzume::game::{Action, Game, Phase, Player, RoundOutcome};
use suzume::scoring::{Meld, MeldKind};
use suzume::tile::Tile;

#[derive(Clone, PartialEq, Eq)]
struct Position {
    players: [Player; 4],
    phase: Phase,
    scores: [i32; 4],
    pot: i32,
    serial: u64,
    draw: Option<(usize, Tile, bool)>,
    dora: Vec<Tile>,
    next_actor: Option<usize>,
}
impl Position {
    fn capture(game: &Game) -> Self {
        Self {
            players: game.players().clone(),
            phase: game.phase().clone(),
            scores: game.scores(),
            pot: game.riichi_pot(),
            serial: game.round_serial(),
            draw: game.last_draw(),
            dora: game.dora_indicators(),
            next_actor: game.next_actor(),
        }
    }
}

/// Produces plain UTF-8 mjson. Full records reveal all hands and draws; choose
/// `public=true` for a spectator stream. This format does not retain core seeds.
pub struct MjaiWriter<W: Write> {
    writer: W,
    previous: Position,
    config: AppConfig,
    effective_ai: [Option<EffectiveAiConfig>; 4],
    sequence: u32,
    public: bool,
    poisoned: bool,
}

impl<W: Write> MjaiWriter<W> {
    pub fn new(writer: W, config: &AppConfig, game: &Game, public: bool) -> Result<Self, ApiError> {
        config.validate()?;
        game.validate().map_err(|e| error(e.to_string()))?;
        if !game.history().is_empty() || game.seed() != config.game.seed.value() {
            return Err(error(
                "mjson logging must start at the configured initial game",
            ));
        }
        let mut result = Self {
            writer,
            previous: Position::capture(game),
            config: config.clone(),
            effective_ai: config.effective_ai()?,
            sequence: 0,
            public,
            poisoned: false,
        };
        let mut start = value(Event::StartGame {
            names: Some(std::array::from_fn(|i| game.players()[i].name.clone())),
        })?;
        start["aka_flag"] = json!(false);
        start["kyoku_first"] = json!(0);
        start["_suzume"] = json!({"version":1,"engine":crate::ENGINE_VERSION,
            "information":if public {"public"} else {"full"},
            "rules_sha256":suzume::rule_constants::DECLARATION_SHA256});
        result.write_value(&start)?;
        for event in result.round_start(game)? {
            result.write_value(&value(event)?)?;
        }
        Ok(result)
    }

    /// Call exactly once after each successful core apply, including Pass and
    /// NextRound. Rejected operations and unselected reactions emit no events.
    pub fn record(
        &mut self,
        actor: usize,
        action: &Action,
        after: &Game,
        decision: Option<&DecisionView>,
    ) -> Result<(), ApiError> {
        self.ensure_writable()?;
        seat(actor)?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("operation count exhausted"))?;
        if self.previous.next_actor != Some(actor)
            || after.seed() != self.config.game.seed.value()
            || after.history().len() != sequence as usize
            || sequence > self.config.game.max_steps
            || !after
                .history()
                .last()
                .is_some_and(|last| last.player == actor && &last.action == action)
        {
            return Err(error(
                "mjson record must describe the next applied operation of the same game",
            ));
        }
        after.validate().map_err(|e| error(e.to_string()))?;
        let wire = WireAction::from_core(action).ok_or_else(|| error("unsupported core action"))?;
        validate_decision(decision, &wire, self.effective_ai[actor].as_ref())?;
        let events = self.events(actor, action, after)?;
        // Validate and serialize all rows before starting this operation's I/O.
        let rows: Vec<Value> = events.into_iter().map(value).collect::<Result<_, _>>()?;
        for row in rows {
            self.write_value(&row)?;
        }
        self.previous = Position::capture(after);
        self.sequence = sequence;
        Ok(())
    }

    pub fn finish(mut self, game: &Game, reason: EndReason) -> Result<W, ApiError> {
        self.ensure_writable()?;
        if game.seed() != self.config.game.seed.value()
            || game.history().len() != self.sequence as usize
            || Position::capture(game) != self.previous
        {
            return Err(error(
                "mjson footer game differs from the last recorded position",
            ));
        }
        validate_reason(game, self.sequence, self.config.game.max_steps, reason)?;
        let mut end = value(Event::EndGame)?;
        end["scores"] = json!(game.scores());
        end["_suzume"] = json!({"reason":reason,"action_count":self.sequence,
            "kyotaku":game.riichi_pot()/1000});
        self.write_value(&end)?;
        self.writer
            .flush()
            .map_err(|e| error(format!("mjson flush failed: {e}")))?;
        Ok(self.writer)
    }

    fn round_start(&self, game: &Game) -> Result<Vec<Event>, ApiError> {
        let (actor, drawn, _) = game
            .last_draw()
            .ok_or_else(|| error("new round has no dealer draw"))?;
        let mut hands: [Vec<Tile>; 4] = std::array::from_fn(|i| game.players()[i].hand.clone());
        let index = hands[actor]
            .iter()
            .position(|&tile| tile == drawn)
            .ok_or_else(|| error("dealer draw is absent from hand"))?;
        hands[actor].remove(index);
        if hands.iter().any(|hand| hand.len() != 13) {
            return Err(error("mjson start_kyoku requires thirteen tiles per seat"));
        }
        Ok(vec![
            Event::StartKyoku {
                bakaze: Pai::new("E").expect("constant wind"),
                dora_marker: game.dora_indicators()[0].into(),
                kyoku: u8::try_from(game.round() + 1)
                    .map_err(|_| error("round number is too large"))?,
                honba: game.honba(),
                kyotaku: (game.riichi_pot() / 1000) as u32,
                oya: seat(game.dealer())?,
                scores: game.scores(),
                tehais: hands.map(|hand| hand.into_iter().map(|tile| self.hidden(tile)).collect()),
            },
            Event::Tsumo {
                actor: seat(actor)?,
                pai: self.hidden(drawn),
            },
        ])
    }

    fn hidden(&self, tile: Tile) -> Pai {
        if self.public {
            Pai::unknown()
        } else {
            tile.into()
        }
    }

    fn events(&self, actor: usize, action: &Action, after: &Game) -> Result<Vec<Event>, ApiError> {
        if after.round_serial() != self.previous.serial {
            return self.round_start(after);
        }
        let mut events = Vec::new();
        let actor_id = seat(actor)?;
        if let Action::Discard(index) | Action::Riichi(index) = action {
            let discarded = *self.previous.players[actor]
                .hand
                .get(*index)
                .ok_or_else(|| error("discard index is out of range"))?;
            if matches!(action, Action::Riichi(_)) {
                events.push(Event::Reach { actor: actor_id });
            }
            events.push(Event::Dahai {
                actor: actor_id,
                pai: discarded.into(),
                // Core tile identity is the tile kind; identical copies are indistinguishable.
                tsumogiri: self
                    .previous
                    .draw
                    .is_some_and(|(seat, tile, _)| seat == actor && tile == discarded),
            });
        }
        let mut before_scores = self.previous.scores;
        for (index, player) in after.players().iter().enumerate() {
            if player.riichi && !self.previous.players[index].riichi {
                events.push(Event::ReachAccepted {
                    actor: seat(index)?,
                });
                before_scores[index] -= 1000;
            }
        }
        // Reach is accepted before any caller acquires the declaration discard,
        // independently of the numerical ordering of the two seats.
        for (index, player) in after.players().iter().enumerate() {
            for meld in player
                .melds
                .iter()
                .skip(self.previous.players[index].melds.len())
            {
                events.push(self.meld(index, meld)?);
            }
        }
        for tile in after
            .dora_indicators()
            .iter()
            .skip(self.previous.dora.len())
        {
            events.push(Event::Dora {
                dora_marker: (*tile).into(),
            });
        }
        if matches!(after.phase(), Phase::Discard { .. })
            && (matches!(self.previous.phase, Phase::Reaction { .. })
                || matches!(action, Action::ClosedKan(_)))
        {
            if let Some((actor, tile, _)) = after.last_draw() {
                events.push(Event::Tsumo {
                    actor: seat(actor)?,
                    pai: self.hidden(tile),
                });
            }
        }
        if let Phase::RoundEnd(result) = after.phase() {
            if !matches!(self.previous.phase, Phase::RoundEnd(_)) {
                let deltas = Some(std::array::from_fn(|i| {
                    after.scores()[i] - before_scores[i]
                }));
                events.push(match result.outcome {
                    RoundOutcome::Win { winner, from, .. } => Event::Hora {
                        actor: seat(winner)?,
                        target: seat(from.unwrap_or(winner))?,
                        deltas,
                        ura_markers: None,
                    },
                    RoundOutcome::Draw { .. } => Event::Ryukyoku { deltas },
                });
                events.push(Event::EndKyoku);
            }
        }
        Ok(events)
    }

    fn meld(&self, actor: usize, meld: &Meld) -> Result<Event, ApiError> {
        let actor = seat(actor)?;
        if meld.kind == MeldKind::ClosedKan {
            let consumed: [Pai; 4] = meld
                .tiles
                .iter()
                .copied()
                .map(Pai::from)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| error("closed kan requires four tiles"))?;
            return Ok(Event::Ankan { actor, consumed });
        }
        let (discarder, discarded) = match self.previous.phase {
            Phase::Reaction {
                discarder, tile, ..
            } => (discarder, tile),
            _ => return Err(error("open meld has no preceding discard")),
        };
        if meld.from != Some(discarder) {
            return Err(error("meld target differs from the previous discard"));
        }
        let mut consumed = meld.tiles.clone();
        let index = consumed
            .iter()
            .position(|&tile| tile == discarded)
            .ok_or_else(|| error("meld lacks its called tile"))?;
        consumed.remove(index);
        let consumed: Vec<Pai> = consumed.into_iter().map(Pai::from).collect();
        let target = seat(discarder)?;
        let pai = discarded.into();
        match meld.kind {
            MeldKind::Chi => Ok(Event::Chi {
                actor,
                target,
                pai,
                consumed: consumed
                    .try_into()
                    .map_err(|_| error("chi requires two consumed tiles"))?,
            }),
            MeldKind::Pon => Ok(Event::Pon {
                actor,
                target,
                pai,
                consumed: consumed
                    .try_into()
                    .map_err(|_| error("pon requires two consumed tiles"))?,
            }),
            MeldKind::OpenKan => Ok(Event::Daiminkan {
                actor,
                target,
                pai,
                consumed: consumed
                    .try_into()
                    .map_err(|_| error("open kan requires three consumed tiles"))?,
            }),
            MeldKind::ClosedKan => unreachable!("closed kan returned above"),
        }
    }

    fn ensure_writable(&self) -> Result<(), ApiError> {
        if self.poisoned {
            Err(error(
                "writer is unusable after an incomplete I/O operation",
            ))
        } else {
            Ok(())
        }
    }

    fn write_value(&mut self, value: &Value) -> Result<(), ApiError> {
        let mut bytes = serde_json::to_vec(value).map_err(|e| error(e.to_string()))?;
        if bytes.len() > LogLimits::default().max_line_bytes {
            return Err(error("mjson event exceeds the line byte limit"));
        }
        bytes.push(b'\n');
        if let Err(e) = self.writer.write_all(&bytes) {
            self.poisoned = true;
            return Err(error(format!("mjson write failed: {e}")));
        }
        Ok(())
    }
}

fn value(event: Event) -> Result<Value, ApiError> {
    serde_json::to_value(event).map_err(|e| error(e.to_string()))
}
