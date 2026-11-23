use super::{
    error, position_sha256, seat, validate_decision, validate_reason, EndReason, LogRecord,
    PublicAction, PublicController, PublicFooter, PublicHeader, PublicStep, ReplayAction,
    ReplayFooter, ReplayHeader, LOG_SESSION_ID, SHUFFLE_ALGORITHM,
};
use crate::config::{AppConfig, EffectiveAiConfig, LogFormat};
use crate::decision::DecisionView;
use crate::types::{ApiError, Version, WireAction};
use crate::view::GameView;
use crate::ENGINE_VERSION;
use std::io::Write;
use suzume::game::{Action, Game};
use suzume::rule_constants::DECLARATION_SHA256;

/// Selects the configured canonical mjson stream or an explicit legacy format.
/// Native mjson is intended for event analysis; legacy replay retains exact core operations.
pub struct LogWriter<W: Write> {
    inner: WriterKind<W>,
}
enum WriterKind<W: Write> {
    Mjai(Box<crate::mjai::MjaiWriter<W>>),
    Legacy(Box<LegacyLogWriter<W>>),
}
impl<W: Write> LogWriter<W> {
    pub fn new(writer: W, config: &AppConfig, game: &Game) -> Result<Self, ApiError> {
        let inner = match config.logging.format {
            LogFormat::Mjai | LogFormat::MjaiPublic => {
                WriterKind::Mjai(Box::new(crate::mjai::MjaiWriter::new(
                    writer,
                    config,
                    game,
                    config.logging.format == LogFormat::MjaiPublic,
                )?))
            }
            LogFormat::Replay | LogFormat::Public => {
                WriterKind::Legacy(Box::new(LegacyLogWriter::new(writer, config, game)?))
            }
        };
        Ok(Self { inner })
    }
    pub fn record(
        &mut self,
        actor: usize,
        action: &Action,
        after: &Game,
        decision: Option<&DecisionView>,
    ) -> Result<(), ApiError> {
        match &mut self.inner {
            WriterKind::Mjai(writer) => writer.record(actor, action, after, decision),
            WriterKind::Legacy(writer) => writer.record(actor, action, after, decision),
        }
    }
    pub fn finish(self, game: &Game, reason: EndReason) -> Result<W, ApiError> {
        match self.inner {
            WriterKind::Mjai(writer) => (*writer).finish(game, reason),
            WriterKind::Legacy(writer) => (*writer).finish(game, reason),
        }
    }
}

#[derive(Clone, Copy)]
enum LegacyFormat {
    Replay,
    Public,
}

/// Writes each completed core operation once. The caller supplies the same
/// evolving Game; replay independently checks every recorded transition.
/// Validation errors leave the writer reusable. Partial I/O makes it unusable.
struct LegacyLogWriter<W: Write> {
    writer: W,
    format: LegacyFormat,
    config: AppConfig,
    effective_ai: [Option<EffectiveAiConfig>; 4],
    sequence: u32,
    next_actor: Option<usize>,
    previous_position: Option<String>,
    poisoned: bool,
}

impl<W: Write> LegacyLogWriter<W> {
    pub fn new(writer: W, config: &AppConfig, game: &Game) -> Result<Self, ApiError> {
        config.validate().map_err(|e| error(e.to_string()))?;
        game.validate().map_err(|e| error(e.to_string()))?;
        if !game.history().is_empty() || config.game.seed.value() != game.seed() {
            return Err(error(
                "logging must start at the initial game with the configured seed",
            ));
        }
        let format = match config.logging.format {
            LogFormat::Replay => LegacyFormat::Replay,
            LogFormat::Public => LegacyFormat::Public,
            _ => return Err(error("legacy writer requires replay or public format")),
        };
        let effective_ai = config.effective_ai().map_err(|e| error(e.to_string()))?;
        let previous_position = match format {
            LegacyFormat::Replay => Some(position_sha256(game, 0)?),
            LegacyFormat::Public => None,
        };
        let header = match format {
            LegacyFormat::Replay => LogRecord::ReplayHeader(ReplayHeader {
                version: Version,
                engine: ENGINE_VERSION.into(),
                rules_sha256: DECLARATION_SHA256.into(),
                shuffle: SHUFFLE_ALGORITHM.into(),
                seed: game.seed().into(),
                config: config.clone(),
                effective_ai: effective_ai.clone(),
                initial_position_sha256: previous_position
                    .clone()
                    .expect("replay format has a position digest"),
            }),
            LegacyFormat::Public => LogRecord::PublicHeader(PublicHeader {
                version: Version,
                engine: ENGINE_VERSION.into(),
                rules_sha256: DECLARATION_SHA256.into(),
                shuffle: SHUFFLE_ALGORITHM.into(),
                max_steps: config.game.max_steps,
                players: std::array::from_fn(|index| match &effective_ai[index] {
                    Some(config) => PublicController::Ai {
                        config: config.into(),
                    },
                    None => PublicController::Human,
                }),
                initial: GameView::from_game(game, None, LOG_SESSION_ID, 0),
            }),
        };
        let mut value = Self {
            writer,
            format,
            config: config.clone(),
            effective_ai,
            sequence: 0,
            next_actor: game.next_actor(),
            previous_position,
            poisoned: false,
        };
        value.write_record(&header)?;
        Ok(value)
    }

    pub fn record(
        &mut self,
        actor: usize,
        action: &Action,
        after: &Game,
        decision: Option<&DecisionView>,
    ) -> Result<(), ApiError> {
        self.ensure_writable()?;
        let actor_id = seat(actor)?;
        if self.next_actor != Some(actor) {
            return Err(error(
                "record actor is not the next actor of the previous position",
            ));
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("sequence exhausted"))?;
        if sequence > self.config.game.max_steps {
            return Err(error(
                "operation limit reached; finish the log before another record",
            ));
        }
        after.validate().map_err(|e| error(e.to_string()))?;
        if after.seed() != self.config.game.seed.value()
            || after.history().len() != sequence as usize
            || !after
                .history()
                .last()
                .is_some_and(|last| last.player == actor && &last.action == action)
        {
            return Err(error(
                "record must describe the next applied operation of the configured game",
            ));
        }
        let wire = WireAction::from_core(action)
            .ok_or_else(|| error("core action is outside the log domain"))?;
        validate_decision(decision, &wire, self.effective_ai[actor].as_ref())?;
        let after_position = match self.format {
            LegacyFormat::Replay => Some(position_sha256(after, sequence)?),
            LegacyFormat::Public => None,
        };
        let record = match self.format {
            LegacyFormat::Replay => LogRecord::ReplayAction(ReplayAction {
                sequence,
                revision: sequence,
                actor: actor_id,
                action: wire,
                before_sha256: self.previous_position.clone().expect("replay position"),
                after_sha256: after_position.clone().expect("replay position"),
                decision: decision.cloned(),
            }),
            LegacyFormat::Public => LogRecord::PublicStep(PublicStep {
                sequence,
                revision: sequence,
                actor: actor_id,
                action: public_action(actor, action, after)?,
                view: GameView::from_game(after, None, LOG_SESSION_ID, sequence),
            }),
        };
        self.write_record(&record)?;
        self.sequence = sequence;
        self.next_actor = after.next_actor();
        self.previous_position = after_position;
        Ok(())
    }

    /// Writes the only footer and flushes the destination. Any returned error
    /// means the output must not be published as a complete log.
    pub fn finish(mut self, game: &Game, reason: EndReason) -> Result<W, ApiError> {
        self.ensure_writable()?;
        if game.seed() != self.config.game.seed.value()
            || game.history().len() != self.sequence as usize
        {
            return Err(error(
                "footer game does not match the recorded operation count",
            ));
        }
        game.validate().map_err(|e| error(e.to_string()))?;
        validate_reason(game, self.sequence, self.config.game.max_steps, reason)?;
        let footer = match self.format {
            LegacyFormat::Replay => {
                let digest = position_sha256(game, self.sequence)?;
                if self.previous_position.as_ref() != Some(&digest) {
                    return Err(error(
                        "footer game does not match the last recorded position",
                    ));
                }
                LogRecord::ReplayFooter(ReplayFooter {
                    action_count: self.sequence,
                    revision: self.sequence,
                    reason,
                    scores: game.scores(),
                    riichi_pot: game.riichi_pot(),
                    final_position_sha256: digest,
                })
            }
            LegacyFormat::Public => LogRecord::PublicFooter(PublicFooter {
                action_count: self.sequence,
                revision: self.sequence,
                reason,
                final_view: GameView::from_game(game, None, LOG_SESSION_ID, self.sequence),
            }),
        };
        self.write_record(&footer)?;
        self.writer
            .flush()
            .map_err(|e| error(format!("log flush failed: {e}")))?;
        Ok(self.writer)
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

    fn write_record(&mut self, record: &LogRecord) -> Result<(), ApiError> {
        let mut bytes = serde_json::to_vec(record).map_err(|e| error(e.to_string()))?;
        if bytes.len() > super::LogLimits::default().max_line_bytes {
            return Err(error("serialized record exceeds the default line limit"));
        }
        bytes.push(b'\n');
        if let Err(error_value) = self.writer.write_all(&bytes) {
            self.poisoned = true;
            return Err(error(format!("log write failed: {error_value}")));
        }
        Ok(())
    }
}

fn public_action(actor: usize, action: &Action, after: &Game) -> Result<PublicAction, ApiError> {
    Ok(match action {
        Action::Discard(_) | Action::Riichi(_) => {
            let tile = after.players()[actor]
                .discards
                .last()
                .ok_or_else(|| error("applied discard has no public tile"))?
                .tile
                .into();
            if matches!(action, Action::Riichi(_)) {
                PublicAction::Riichi { tile }
            } else {
                PublicAction::Discard { tile }
            }
        }
        Action::ClosedKan(tile) => PublicAction::ClosedKan {
            tile: (*tile).into(),
        },
        Action::Tsumo => PublicAction::Tsumo,
        Action::NextRound => PublicAction::NextRound,
        Action::Ron | Action::Pon | Action::Chi(_) | Action::OpenKan | Action::Pass => {
            PublicAction::ResponseSubmitted
        }
    })
}
