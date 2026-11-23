use super::{
    error, position_sha256, validate_decision, validate_reason, DatasetTransition, LogLimits,
    LogRecord, ReplayFooter, ReplayHeader, ReplayResult, LOG_SESSION_ID, SHUFFLE_ALGORITHM,
};
use crate::actions::action_id;
use crate::config::LogFormat;
use crate::environment::EncodedObservation;
use crate::types::{ApiError, Version};
use crate::ENGINE_VERSION;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::fmt;
use std::io::BufRead;
use suzume::game::{Game, Phase};
use suzume::rule_constants::DECLARATION_SHA256;

/// Validates every input record and returns the actual reconstructed core state.
/// No complete-log success is returned before a footer and clean EOF are read.
pub fn replay<R: BufRead>(reader: &mut R, limits: LogLimits) -> Result<ReplayResult, ApiError> {
    read_replay::<R, fn(&DatasetTransition) -> Result<(), ApiError>>(reader, limits, None)
}

/// Emits one transition at a time with at most one pending final row. Previously
/// emitted rows are provisional until this function returns Ok: a later malformed
/// record, footer, I/O operation, or consumer failure still invalidates the input.
pub fn export_transitions<R: BufRead, F>(
    reader: &mut R,
    limits: LogLimits,
    mut consumer: F,
) -> Result<ReplayResult, ApiError>
where
    F: FnMut(&DatasetTransition) -> Result<(), ApiError>,
{
    read_replay(reader, limits, Some(&mut consumer))
}

fn read_replay<R: BufRead, F>(
    reader: &mut R,
    limits: LogLimits,
    mut consumer: Option<&mut F>,
) -> Result<ReplayResult, ApiError>
where
    F: FnMut(&DatasetTransition) -> Result<(), ApiError>,
{
    let mut records = Records::new(reader, limits)?;
    let header = match records.next()? {
        Some(LogRecord::ReplayHeader(header)) => header,
        Some(LogRecord::PublicHeader(_)) => return Err(error("public logs cannot be replayed")),
        _ => return Err(error("the first record must be a replay_header")),
    };
    let mut game = validate_header(&header)?;
    let mut digest = header.initial_position_sha256.clone();
    let mut sequence = 0_u32;
    let mut pending: Option<DatasetTransition> = None;
    loop {
        match records.next()? {
            Some(LogRecord::ReplayAction(record)) => {
                let expected = sequence
                    .checked_add(1)
                    .ok_or_else(|| error("sequence exhausted"))?;
                if record.sequence != expected
                    || record.revision != expected
                    || expected > header.config.game.max_steps
                {
                    return Err(error(
                        "action sequence/revision must be consecutive and within max_steps",
                    ));
                }
                if record.before_sha256 != digest {
                    return Err(error(
                        "action before_sha256 does not match the replayed position",
                    ));
                }
                let actor = usize::from(record.actor.get());
                if game.next_actor() != Some(actor) {
                    return Err(error("action actor is not the next actor"));
                }
                validate_decision(
                    record.decision.as_ref(),
                    &record.action,
                    header.effective_ai[actor].as_ref(),
                )?;
                let action = record.action.to_core();
                let id = action_id(&game, actor, &action).ok_or_else(|| {
                    error("action cannot be encoded in the shared 189-action space")
                })?;
                let observation = if consumer.is_some() {
                    Some(
                        EncodedObservation::for_actor(&game, LOG_SESSION_ID, sequence)
                            .ok_or_else(|| error("action has no preceding observation"))?,
                    )
                } else {
                    None
                };
                let scores = game.scores();
                let pot = game.riichi_pot();
                game.apply(actor, action)
                    .map_err(|e| error(format!("illegal replay action: {e}")))?;
                sequence = expected;
                digest = position_sha256(&game, sequence)?;
                if record.after_sha256 != digest {
                    return Err(error(
                        "action after_sha256 does not match the replayed position",
                    ));
                }
                if let Some(observation) = observation {
                    let terminated = matches!(game.phase(), Phase::MatchEnd);
                    let next_observation = if terminated {
                        None
                    } else {
                        EncodedObservation::for_actor(&game, LOG_SESSION_ID, sequence)
                    };
                    let next = DatasetTransition {
                        schema_version: Version,
                        sequence,
                        actor: record.actor,
                        observation,
                        action: record.action,
                        action_id: id,
                        rewards: std::array::from_fn(|index| game.scores()[index] - scores[index]),
                        escrow_delta: game.riichi_pot() - pot,
                        next_actor: next_observation
                            .as_ref()
                            .map(|observation| observation.actor),
                        next_observation,
                        final_observation: None,
                        terminated,
                        truncated: false,
                    };
                    if let (Some(previous), Some(callback)) =
                        (pending.take(), consumer.as_deref_mut())
                    {
                        callback(&previous)?;
                    }
                    pending = Some(next);
                }
            }
            Some(LogRecord::ReplayFooter(footer)) => {
                validate_footer(&game, &header, &footer, sequence, &digest)?;
                if records.next()?.is_some() {
                    return Err(error("records after the footer are not permitted"));
                }
                if let (Some(mut final_row), Some(callback)) = (pending, consumer) {
                    final_row.terminated = matches!(game.phase(), Phase::MatchEnd);
                    final_row.truncated = !final_row.terminated;
                    final_row.final_observation = if final_row.truncated {
                        Some(
                            EncodedObservation::for_actor(&game, LOG_SESSION_ID, sequence)
                                .ok_or_else(|| error("truncated game has no final observation"))?,
                        )
                    } else {
                        None
                    };
                    final_row.next_actor = None;
                    final_row.next_observation = None;
                    callback(&final_row)?;
                }
                return Ok(ReplayResult {
                    game,
                    metadata: header,
                    termination: footer,
                });
            }
            None => return Err(error("missing replay_footer; the log is incomplete")),
            _ => return Err(error("unexpected record kind inside a replay log")),
        }
    }
}

fn validate_header(header: &ReplayHeader) -> Result<Game, ApiError> {
    if header.engine != ENGINE_VERSION
        || header.rules_sha256 != DECLARATION_SHA256
        || header.shuffle != SHUFFLE_ALGORITHM
    {
        return Err(error(
            "unsupported engine, rules digest, or shuffle algorithm",
        ));
    }
    header.config.validate().map_err(|e| error(e.to_string()))?;
    if header.config.logging.format != LogFormat::Replay
        || header.seed != header.config.game.seed
        || header.effective_ai
            != header
                .config
                .effective_ai()
                .map_err(|e| error(e.to_string()))?
    {
        return Err(error(
            "header seed, replay format, or effective configuration is inconsistent",
        ));
    }
    let game = Game::new(header.seed.value()).map_err(|e| error(e.to_string()))?;
    if position_sha256(&game, 0)? != header.initial_position_sha256 {
        return Err(error(
            "initial_position_sha256 does not match the configured game",
        ));
    }
    Ok(game)
}

fn validate_footer(
    game: &Game,
    header: &ReplayHeader,
    footer: &ReplayFooter,
    sequence: u32,
    digest: &str,
) -> Result<(), ApiError> {
    if footer.action_count != sequence
        || footer.revision != sequence
        || footer.scores != game.scores()
        || footer.riichi_pot != game.riichi_pot()
        || footer.final_position_sha256 != digest
    {
        return Err(error(
            "footer count, scores, escrow, or position digest does not match replay",
        ));
    }
    validate_reason(game, sequence, header.config.game.max_steps, footer.reason)
}

struct Records<'a, R: BufRead> {
    reader: &'a mut R,
    limits: LogLimits,
    total: usize,
    count: usize,
}
impl<'a, R: BufRead> Records<'a, R> {
    fn new(reader: &'a mut R, limits: LogLimits) -> Result<Self, ApiError> {
        if limits.max_line_bytes == 0 || limits.max_records == 0 || limits.max_total_bytes == 0 {
            return Err(error("all log limits must be positive"));
        }
        Ok(Self {
            reader,
            limits,
            total: 0,
            count: 0,
        })
    }

    fn next(&mut self) -> Result<Option<LogRecord>, ApiError> {
        let mut line = Vec::new();
        loop {
            let available = self
                .reader
                .fill_buf()
                .map_err(|e| error(format!("log read failed: {e}")))?;
            if available.is_empty() {
                if line.is_empty() {
                    return Ok(None);
                }
                break;
            }
            if self.count >= self.limits.max_records {
                return Err(error("log record count exceeds the configured limit"));
            }
            let newline = available.iter().position(|&byte| byte == b'\n');
            let content = newline.unwrap_or(available.len());
            let consumed = content + usize::from(newline.is_some());
            if line
                .len()
                .checked_add(content)
                .is_none_or(|size| size > self.limits.max_line_bytes)
            {
                return Err(error("log line exceeds the configured byte limit"));
            }
            let total = self
                .total
                .checked_add(consumed)
                .ok_or_else(|| error("total byte counter overflow"))?;
            if total > self.limits.max_total_bytes {
                return Err(error("log total exceeds the configured byte limit"));
            }
            line.extend_from_slice(&available[..content]);
            self.reader.consume(consumed);
            self.total = total;
            if newline.is_some() {
                break;
            }
        }
        self.count += 1;
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.is_empty() {
            return Err(error(format!("empty JSONL record at line {}", self.count)));
        }
        // Parse the entire line, preserving duplicate detection before serde's
        // internally-tagged enum buffering or a Value conversion could erase it.
        let unique: UniqueValue = serde_json::from_slice(&line)
            .map_err(|e| error(format!("invalid JSON at line {}: {e}", self.count)))?;
        let record = serde_json::from_value(unique.0)
            .map_err(|e| error(format!("invalid log record at line {}: {e}", self.count)))?;
        Ok(Some(record))
    }
}

struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueVisitor).map(Self)
    }
}
struct UniqueVisitor;
impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object keys and integer numeric values")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = input.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key: {key}")));
            }
            let value: UniqueValue = input.next_value()?;
            values.insert(key, value.0);
        }
        Ok(Value::Object(values))
    }
}
