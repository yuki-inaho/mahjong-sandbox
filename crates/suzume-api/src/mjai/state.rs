use super::{
    error, AgentObservation, Event, MeldKind, MjaiDiscard, MjaiLastDiscard, MjaiMeld, MjaiPlayer,
    Pai, ParsedEvent, Summary,
};
use crate::{
    logging::EndReason,
    types::{ApiError, SeatId},
};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    BeforeGame,
    BetweenRounds,
    Playing,
    Outcome,
    Finished,
}

/// A bounded per-round board reconstructed from the record. `apply` is atomic:
/// invalid events do not modify a previously accepted prefix.
#[derive(Clone, Debug)]
pub struct ReplayState {
    phase: Phase,
    hands: [Vec<Pai>; 4],
    players: [MjaiPlayer; 4],
    scores: Option<[i32; 4]>,
    sticks: Option<u32>,
    bakaze: Option<Pai>,
    kyoku: Option<u8>,
    honba: Option<u32>,
    dealer: Option<SeatId>,
    dora_markers: Vec<Pai>,
    last_discard: Option<MjaiLastDiscard>,
    claim_target: Option<SeatId>,
    last_draw: Option<(SeatId, Pai)>,
    turn: Option<SeatId>,
    needs_draw: bool,
    winners: [bool; 4],
    win_target: Option<SeatId>,
    kan_count: u8,
    round_settled: bool,
    summary: Summary,
}

impl Default for ReplayState {
    fn default() -> Self {
        Self {
            phase: Phase::BeforeGame,
            hands: std::array::from_fn(|_| Vec::new()),
            players: std::array::from_fn(|_| MjaiPlayer::default()),
            scores: None,
            sticks: None,
            bakaze: None,
            kyoku: None,
            honba: None,
            dealer: None,
            dora_markers: Vec::new(),
            last_discard: None,
            claim_target: None,
            last_draw: None,
            turn: None,
            needs_draw: true,
            winners: [false; 4],
            win_target: None,
            kan_count: 0,
            round_settled: false,
            summary: Summary::default(),
        }
    }
}

impl ReplayState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn summary(&self) -> &Summary {
        &self.summary
    }
    pub fn is_finished(&self) -> bool {
        self.phase == Phase::Finished
    }

    pub fn observe(&self, viewer: Option<SeatId>) -> AgentObservation {
        AgentObservation {
            viewer,
            own_hand: viewer.map(|seat| self.hands[usize::from(seat.get())].clone()),
            own_last_draw: self
                .last_draw
                .as_ref()
                .and_then(|(seat, pai)| (Some(*seat) == viewer).then(|| pai.clone())),
            players: self.players.clone(),
            scores: self.scores,
            riichi_sticks: self.sticks,
            bakaze: self.bakaze.clone(),
            kyoku: self.kyoku,
            honba: self.honba,
            dealer: self.dealer,
            dora_markers: self.dora_markers.clone(),
            last_discard: self.last_discard.clone(),
            round_ended: self.round_settled,
            game_ended: self.is_finished() && self.summary.complete,
            record_ended: self.is_finished(),
        }
    }

    pub fn apply(&mut self, record: &ParsedEvent) -> Result<(), ApiError> {
        let mut candidate = self.clone();
        candidate
            .apply_inner(record)
            .and_then(|()| candidate.check_tile_inventory())
            .map_err(|message| error(record.line, record.event.kind(), message))?;
        increment(&mut candidate.summary.records)
            .map_err(|message| error(record.line, record.event.kind(), message))?;
        if contains_red(&record.raw) {
            increment(&mut candidate.summary.red_events)
                .map_err(|message| error(record.line, record.event.kind(), message))?;
        }
        candidate.summary.final_scores = candidate.scores;
        candidate.summary.riichi_sticks = candidate.sticks;
        *self = candidate;
        Ok(())
    }

    fn apply_inner(&mut self, record: &ParsedEvent) -> Result<(), String> {
        if self.phase == Phase::Finished {
            return Err("events after end_game are not permitted".into());
        }
        match &record.event {
            Event::StartGame { .. } => {
                self.require_phase(Phase::BeforeGame)?;
                self.phase = Phase::BetweenRounds;
            }
            Event::StartKyoku {
                bakaze,
                dora_marker,
                kyoku,
                honba,
                kyotaku,
                oya,
                scores,
                tehais,
            } => {
                self.require_phase(Phase::BetweenRounds)?;
                if !matches!(bakaze.as_str(), "E" | "S" | "W" | "N") || !(1..=4).contains(kyoku) {
                    return Err("round wind must be E/S/W/N and kyoku must be 1..4".into());
                }
                known(dora_marker)?;
                if tehais.iter().any(|hand| hand.len() != 13) {
                    return Err("four initial hands must contain exactly 13 tiles each".into());
                }
                if self.summary.rounds > 0 {
                    if self.scores.is_some_and(|previous| previous != *scores) {
                        return Err(
                            "start_kyoku scores disagree with the preceding settlement".into()
                        );
                    }
                    if self.sticks.is_some_and(|previous| previous != *kyotaku) {
                        return Err(
                            "start_kyoku kyotaku disagrees with the preceding settlement".into(),
                        );
                    }
                }
                self.hands = tehais.clone();
                self.players = std::array::from_fn(|_| MjaiPlayer {
                    hand_count: 13,
                    ..Default::default()
                });
                self.scores = Some(*scores);
                self.sticks = Some(*kyotaku);
                self.bakaze = Some(bakaze.clone());
                self.kyoku = Some(*kyoku);
                self.honba = Some(*honba);
                self.dealer = Some(*oya);
                self.dora_markers = vec![dora_marker.clone()];
                self.last_discard = None;
                self.claim_target = None;
                self.last_draw = None;
                self.turn = Some(*oya);
                self.needs_draw = true;
                self.winners = [false; 4];
                self.win_target = None;
                self.kan_count = 0;
                self.round_settled = false;
                self.phase = Phase::Playing;
                increment(&mut self.summary.rounds)?;
            }
            Event::Tsumo { actor, pai } => {
                self.require_phase(Phase::Playing)?;
                self.require_turn(*actor, true)?;
                self.require_size(*actor, 13)?;
                self.hands[index(*actor)].push(pai.clone());
                self.sync_count(*actor)?;
                self.last_draw = Some((*actor, pai.clone()));
                self.last_discard = None;
                self.claim_target = None;
                self.needs_draw = false;
            }
            Event::Dahai {
                actor,
                pai,
                tsumogiri,
            } => {
                self.require_phase(Phase::Playing)?;
                self.require_turn(*actor, false)?;
                self.require_size(*actor, 14)?;
                known(pai)?;
                let consume_unknown = if *tsumogiri {
                    let drawn = self
                        .last_draw
                        .as_ref()
                        .filter(|(seat, _)| seat == actor)
                        .ok_or("tsumogiri requires a preceding draw by this actor")?;
                    if !drawn.1.is_unknown() && drawn.1 != *pai {
                        return Err("tsumogiri tile differs from the preceding draw".into());
                    }
                    drawn.1.is_unknown()
                } else {
                    // A hand discard cannot consume the just-drawn physical
                    // tile. If it is the only known matching spelling, consume
                    // an unknown original tile and retain the known draw. With
                    // another identical known copy, removing either copy gives
                    // the same observable multiset.
                    let hand = &self.hands[index(*actor)];
                    let only_drawn_match = self
                        .last_draw
                        .as_ref()
                        .is_some_and(|(seat, drawn)| seat == actor && drawn == pai)
                        && hand.iter().filter(|tile| *tile == pai).count() == 1;
                    if only_drawn_match && !hand.iter().any(Pai::is_unknown) {
                        return Err(
                            "tsumogiri=false cannot discard the sole just-drawn tile".into()
                        );
                    }
                    only_drawn_match
                };
                self.remove(*actor, pai, consume_unknown)?;
                self.players[index(*actor)].discards.push(MjaiDiscard {
                    pai: pai.clone(),
                    tsumogiri: *tsumogiri,
                    called: false,
                });
                self.last_discard = Some(MjaiLastDiscard {
                    actor: *actor,
                    pai: pai.clone(),
                });
                self.claim_target = Some(*actor);
                self.last_draw = None;
                self.turn = SeatId::new((actor.get() + 1) % 4);
                self.needs_draw = true;
            }
            Event::Chi {
                actor,
                target,
                pai,
                consumed,
            } => self.call(*actor, *target, pai, consumed, MeldKind::Chi)?,
            Event::Pon {
                actor,
                target,
                pai,
                consumed,
            } => self.call(*actor, *target, pai, consumed, MeldKind::Pon)?,
            Event::Daiminkan {
                actor,
                target,
                pai,
                consumed,
            } => self.call(*actor, *target, pai, consumed, MeldKind::Daiminkan)?,
            Event::Ankan { actor, consumed } => {
                self.require_phase(Phase::Playing)?;
                self.require_turn(*actor, false)?;
                self.require_size(*actor, 14)?;
                same_kind(consumed)?;
                for pai in consumed {
                    self.remove(*actor, pai, false)?;
                }
                self.players[index(*actor)].melds.push(MjaiMeld {
                    kind: MeldKind::Ankan,
                    tiles: consumed.to_vec(),
                    target: None,
                });
                self.add_kan()?;
                self.last_draw = None;
                self.last_discard = None;
                self.claim_target = Some(*actor);
                self.needs_draw = true;
                increment(&mut self.summary.calls)?;
            }
            Event::Kakan {
                actor,
                pai,
                consumed,
            } => {
                self.require_phase(Phase::Playing)?;
                self.require_turn(*actor, false)?;
                self.require_size(*actor, 14)?;
                let mut tiles = consumed.to_vec();
                tiles.push(pai.clone());
                same_kind(&tiles)?;
                let meld_index = self.players[index(*actor)]
                    .melds
                    .iter()
                    .position(|meld| {
                        let mut old = meld.tiles.clone();
                        let mut expected = consumed.to_vec();
                        old.sort();
                        expected.sort();
                        meld.kind == MeldKind::Pon && old == expected
                    })
                    .ok_or("kakan consumed tiles do not match an existing pon")?;
                self.remove(*actor, pai, false)?;
                let meld = &mut self.players[index(*actor)].melds[meld_index];
                meld.kind = MeldKind::Kakan;
                meld.tiles.push(pai.clone());
                self.add_kan()?;
                self.last_draw = None;
                self.last_discard = None;
                self.claim_target = Some(*actor);
                self.needs_draw = true;
                increment(&mut self.summary.calls)?;
            }
            Event::Dora { dora_marker } => {
                self.require_phase(Phase::Playing)?;
                known(dora_marker)?;
                if self.dora_markers.len() > usize::from(self.kan_count) {
                    return Err("dora event has no preceding kan or exceeds five indicators".into());
                }
                self.dora_markers.push(dora_marker.clone());
            }
            Event::Reach { actor } => {
                self.require_phase(Phase::Playing)?;
                self.require_turn(*actor, false)?;
                let player = &mut self.players[index(*actor)];
                if player.reach_declared {
                    return Err("duplicate reach declaration".into());
                }
                player.reach_declared = true;
            }
            Event::ReachAccepted { actor } => {
                self.require_phase(Phase::Playing)?;
                let player = &mut self.players[index(*actor)];
                if !player.reach_declared
                    || player.reach_accepted
                    || self.last_discard.as_ref().map(|d| d.actor) != Some(*actor)
                {
                    return Err("reach_accepted requires one unaccepted declaration followed by its discard".into());
                }
                if let Some(scores) = &mut self.scores {
                    if scores[index(*actor)] < 1000 {
                        return Err("reach deposit requires at least 1000 points".into());
                    }
                    scores[index(*actor)] -= 1000;
                }
                self.sticks = self
                    .sticks
                    .map(|sticks| sticks.checked_add(1).ok_or("riichi stick counter overflow"))
                    .transpose()?;
                player.reach_accepted = true;
            }
            Event::Hora {
                actor,
                target,
                deltas,
                ura_markers,
            } => {
                if !matches!(self.phase, Phase::Playing | Phase::Outcome) {
                    return Err(
                        "hora requires a live round or another ron on the same discard".into(),
                    );
                }
                if self.winners[index(*actor)] {
                    return Err("duplicate hora winner".into());
                }
                if self.phase == Phase::Outcome {
                    // A target that is already a winner identifies a preceding
                    // self-draw win, which cannot be followed by another ron.
                    if self.win_target != Some(*target)
                        || actor == target
                        || self.winners[index(*target)]
                    {
                        return Err("multiple hora events must all be ron on one target".into());
                    }
                } else if actor == target {
                    self.require_turn(*actor, false)?;
                    if self.last_draw.as_ref().map(|draw| draw.0) != Some(*actor) {
                        return Err("self-draw win requires this actor's preceding draw".into());
                    }
                } else if self.claim_target != Some(*target) {
                    return Err("ron target does not match the preceding discard or kan".into());
                }
                if let Some(markers) = ura_markers {
                    if markers.len() > 5 {
                        return Err("at most five ura markers are supported".into());
                    }
                    for pai in markers {
                        known(pai)?;
                    }
                }
                self.settle(*deltas)?;
                self.winners[index(*actor)] = true;
                self.win_target = Some(*target);
                self.phase = Phase::Outcome;
                self.round_settled = true;
                increment(&mut self.summary.wins)?;
            }
            Event::Ryukyoku { deltas } => {
                self.require_phase(Phase::Playing)?;
                self.settle(*deltas)?;
                self.phase = Phase::Outcome;
                self.round_settled = true;
                increment(&mut self.summary.draws)?;
            }
            Event::EndKyoku => {
                self.require_phase(Phase::Outcome)?;
                self.phase = Phase::BetweenRounds;
            }
            Event::EndGame => {
                if self.phase == Phase::BeforeGame {
                    return Err("end_game requires start_game".into());
                }
                let metadata = record.raw.get("_suzume");
                if metadata.is_some_and(|value| !value.is_object()) {
                    return Err("_suzume must be an object".into());
                }
                let reason = match metadata.and_then(|value| value.get("reason")) {
                    None => EndReason::Completed,
                    Some(Value::String(reason)) if reason == "completed" => EndReason::Completed,
                    Some(Value::String(reason)) if reason == "interrupted" => {
                        EndReason::Interrupted
                    }
                    Some(Value::String(reason)) if reason == "truncated" => EndReason::Truncated,
                    _ => return Err("unsupported _suzume.reason".into()),
                };
                if reason == EndReason::Completed && self.phase != Phase::BetweenRounds {
                    return Err("completed end_game requires end_kyoku after the outcome".into());
                }
                if let Some(complete) = metadata.and_then(|value| value.get("complete")) {
                    if complete.as_bool() != Some(reason == EndReason::Completed) {
                        return Err("_suzume.complete contradicts reason".into());
                    }
                }
                self.final_settlement(&record.raw)?;
                self.summary.termination = reason;
                self.summary.complete = reason == EndReason::Completed;
                self.phase = Phase::Finished;
            }
            Event::None => {
                if self.phase == Phase::BeforeGame {
                    return Err("first event must be start_game".into());
                }
            }
        }
        Ok(())
    }

    fn require_phase(&self, expected: Phase) -> Result<(), String> {
        (self.phase == expected).then_some(()).ok_or_else(|| {
            format!(
                "unexpected event in {:?}; expected {:?}",
                self.phase, expected
            )
        })
    }
    fn require_turn(&self, actor: SeatId, needs_draw: bool) -> Result<(), String> {
        (self.turn == Some(actor) && self.needs_draw == needs_draw)
            .then_some(())
            .ok_or_else(|| {
                "actor or draw/discard ordering disagrees with the preceding event".into()
            })
    }
    fn require_size(&self, actor: SeatId, expected: usize) -> Result<(), String> {
        (self.hands[index(actor)].len() + 3 * self.players[index(actor)].melds.len() == expected)
            .then_some(())
            .ok_or_else(|| format!("structural hand size must be {expected}"))
    }
    fn sync_count(&mut self, actor: SeatId) -> Result<(), String> {
        self.players[index(actor)].hand_count =
            u8::try_from(self.hands[index(actor)].len()).map_err(|_| "hand size overflow")?;
        Ok(())
    }
    fn remove(&mut self, actor: SeatId, pai: &Pai, unknown_only: bool) -> Result<(), String> {
        let hand = &mut self.hands[index(actor)];
        let found = if unknown_only {
            None
        } else {
            hand.iter().position(|tile| tile == pai)
        }
        .or_else(|| hand.iter().position(Pai::is_unknown))
        .ok_or_else(|| {
            format!(
                "actor {} does not hold {} or an unknown tile",
                actor.get(),
                pai.as_str()
            )
        })?;
        hand.remove(found);
        self.sync_count(actor)
    }
    fn call(
        &mut self,
        actor: SeatId,
        target: SeatId,
        pai: &Pai,
        consumed: &[Pai],
        kind: MeldKind,
    ) -> Result<(), String> {
        self.require_phase(Phase::Playing)?;
        self.require_size(actor, 13)?;
        let discarded = self
            .last_discard
            .as_ref()
            .ok_or("call requires a preceding discard")?;
        if actor == target || discarded.actor != target || discarded.pai != *pai || !self.needs_draw
        {
            return Err("call does not match the preceding discard".into());
        }
        let mut tiles = consumed.to_vec();
        tiles.push(pai.clone());
        if kind == MeldKind::Chi {
            if actor.get() != (target.get() + 1) % 4 {
                return Err("chi must be from the immediately preceding seat".into());
            }
            let mut ids = tiles
                .iter()
                .map(|tile| known(tile).map(|()| tile.kind_id().expect("known tile")))
                .collect::<Result<Vec<_>, _>>()?;
            ids.sort_unstable();
            if ids[0] >= 27
                || ids[0] / 9 != ids[2] / 9
                || ids[1] != ids[0] + 1
                || ids[2] != ids[1] + 1
            {
                return Err("chi tiles must form a suited sequence".into());
            }
        } else {
            same_kind(&tiles)?;
        }
        for tile in consumed {
            self.remove(actor, tile, false)?;
        }
        self.players[index(target)]
            .discards
            .last_mut()
            .ok_or("called river is empty")?
            .called = true;
        self.players[index(actor)].melds.push(MjaiMeld {
            kind,
            tiles,
            target: Some(target),
        });
        self.turn = Some(actor);
        self.last_draw = None;
        self.last_discard = None;
        self.claim_target = None;
        self.needs_draw = kind == MeldKind::Daiminkan;
        if self.needs_draw {
            self.add_kan()?;
        }
        increment(&mut self.summary.calls)
    }
    fn add_kan(&mut self) -> Result<(), String> {
        if self.kan_count >= 4 {
            return Err("more than four kans in a round".into());
        }
        self.kan_count += 1;
        Ok(())
    }
    fn settle(&mut self, deltas: Option<[i32; 4]>) -> Result<(), String> {
        let Some(deltas) = deltas else {
            self.scores = None;
            self.sticks = None;
            return Ok(());
        };
        let total: i64 = deltas.iter().map(|delta| i64::from(*delta)).sum();
        if total < 0 || total % 1000 != 0 {
            return Err("settlement deltas must conserve points plus whole riichi deposits".into());
        }
        if let Some(sticks) = self.sticks {
            let paid = u32::try_from(total / 1000).map_err(|_| "settlement deposit overflow")?;
            self.sticks = Some(
                sticks
                    .checked_sub(paid)
                    .ok_or("settlement pays more riichi deposits than available")?,
            );
        }
        if let Some(scores) = &mut self.scores {
            for (score, delta) in scores.iter_mut().zip(deltas) {
                *score = score.checked_add(delta).ok_or("score overflow")?;
            }
        }
        Ok(())
    }
    fn final_settlement(&mut self, raw: &Value) -> Result<(), String> {
        let final_sticks = raw
            .get("_suzume")
            .and_then(|value| value.get("kyotaku"))
            .map(|value| {
                serde_json::from_value::<u32>(value.clone())
                    .map_err(|_| "_suzume.kyotaku must be u32".to_owned())
            })
            .transpose()?;
        let final_scores = raw
            .get("scores")
            .map(|value| {
                serde_json::from_value::<[i32; 4]>(value.clone())
                    .map_err(|_| "end_game scores must contain four i32 values".to_owned())
            })
            .transpose()?;
        if let Some(sticks) = final_sticks {
            if self.sticks.is_some_and(|previous| sticks > previous) {
                return Err("end_game cannot increase riichi deposits".into());
            }
            if self.sticks != Some(sticks) && final_scores.is_none() {
                return Err("changed end_game deposits require explicit final scores".into());
            }
        }
        if let Some(scores) = final_scores {
            if let (Some(previous), Some(sticks)) = (self.scores, self.sticks) {
                let remaining = final_sticks.unwrap_or(sticks);
                let payout = i64::from(sticks - remaining) * 1000;
                let changes: Vec<i64> = scores
                    .iter()
                    .zip(previous)
                    .map(|(after, before)| i64::from(*after) - i64::from(before))
                    .collect();
                let winner = changes.iter().position(|delta| *delta > 0);
                let maximum = previous
                    .iter()
                    .max()
                    .copied()
                    .ok_or("missing score vector")?;
                if changes.iter().any(|delta| *delta < 0)
                    || changes.iter().sum::<i64>() != payout
                    || changes.iter().filter(|delta| **delta > 0).count() > 1
                    || winner.is_some_and(|seat| previous[seat] != maximum)
                {
                    return Err(
                        "end_game scores may only award remaining deposits to one leading player"
                            .into(),
                    );
                }
            }
            self.scores = Some(scores);
        }
        if let Some(sticks) = final_sticks {
            self.sticks = Some(sticks);
        }
        Ok(())
    }
    fn check_tile_inventory(&self) -> Result<(), String> {
        const COPIES_PER_KIND: usize = 4;
        let mut counts = [0usize; 34];
        let maximum_tiles = counts.len() * COPIES_PER_KIND;
        let mut physical_tiles = 0;
        let mut count = |pai: &Pai| -> Result<(), String> {
            // Unknown tiles still occupy physical slots. This is a bound on
            // retained inventory, not a reconstruction of the complete wall.
            physical_tiles += 1;
            if physical_tiles > maximum_tiles {
                return Err(format!(
                    "more than {maximum_tiles} physical tiles including unknowns"
                ));
            }
            if let Some(kind) = pai.kind_id() {
                counts[usize::from(kind)] += 1;
                if counts[usize::from(kind)] > COPIES_PER_KIND {
                    return Err(format!("more than four known copies of tile kind {kind}"));
                }
            }
            Ok(())
        };
        for hand in &self.hands {
            for pai in hand {
                count(pai)?;
            }
        }
        for player in &self.players {
            for meld in &player.melds {
                for pai in &meld.tiles {
                    count(pai)?;
                }
            }
            for discard in &player.discards {
                if !discard.called {
                    count(&discard.pai)?;
                }
            }
        }
        for pai in &self.dora_markers {
            count(pai)?;
        }
        Ok(())
    }
}

fn index(seat: SeatId) -> usize {
    usize::from(seat.get())
}
fn increment(value: &mut u32) -> Result<(), String> {
    *value = value.checked_add(1).ok_or("event counter overflow")?;
    Ok(())
}
fn known(pai: &Pai) -> Result<(), String> {
    (!pai.is_unknown())
        .then_some(())
        .ok_or_else(|| "a public tile cannot be unknown".into())
}
fn same_kind(tiles: &[Pai]) -> Result<(), String> {
    for pai in tiles {
        known(pai)?;
    }
    let first = tiles
        .first()
        .and_then(Pai::kind_id)
        .ok_or("meld has no known tile")?;
    tiles
        .iter()
        .all(|pai| pai.kind_id() == Some(first))
        .then_some(())
        .ok_or_else(|| "pon/kan tiles must have the same kind".into())
}
fn contains_red(value: &Value) -> bool {
    match value {
        Value::String(text) => matches!(text.as_str(), "5mr" | "5pr" | "5sr"),
        Value::Array(values) => values.iter().any(contains_red),
        Value::Object(values) => values.values().any(contains_red),
        _ => false,
    }
}
