use super::{error, Event, ParsedEvent, ReplayState, Summary};
use crate::{logging::LogLimits, types::ApiError};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Map, Value};
use std::{fmt, io::BufRead};

/// Bounded structural validation of one event. For order, hand movement and
/// settlement validation, use `analyze` or `visit` on a complete stream.
pub fn parse_event(text: &str) -> Result<Event, ApiError> {
    if text.len() > LogLimits::default().max_line_bytes {
        return Err(error(1, "json", "line byte limit exceeded"));
    }
    Ok(parse_record(text.as_bytes(), 1)?.event)
}

pub fn analyze<R: BufRead + ?Sized>(
    reader: &mut R,
    limits: LogLimits,
) -> Result<Summary, ApiError> {
    visit(reader, limits, |_, _| Ok(()))
}

/// Visits the validated prefix, retaining one raw event and one round's board.
/// A later invalid event/EOF or a callback failure invalidates the whole input;
/// callers must publish derived files only after this function succeeds.
pub fn visit<R: BufRead + ?Sized, F>(
    reader: &mut R,
    limits: LogLimits,
    mut consumer: F,
) -> Result<Summary, ApiError>
where
    F: FnMut(&ParsedEvent, &ReplayState) -> Result<(), ApiError>,
{
    if limits.max_line_bytes == 0 || limits.max_records == 0 || limits.max_total_bytes == 0 {
        return Err(error(1, "limits", "all limits must be positive"));
    }
    let mut state = ReplayState::new();
    let mut count = 0usize;
    let mut total = 0usize;
    loop {
        let mut bytes = Vec::new();
        let mut has_data = false;
        loop {
            let available = reader
                .fill_buf()
                .map_err(|cause| error(count + 1, "io", cause))?;
            if available.is_empty() {
                break;
            }
            has_data = true;
            if count >= limits.max_records {
                return Err(error(count + 1, "limits", "record count exceeds limit"));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let content = newline.unwrap_or(available.len());
            let consumed = content + usize::from(newline.is_some());
            if bytes
                .len()
                .checked_add(content)
                .is_none_or(|length| length > limits.max_line_bytes)
            {
                return Err(error(count + 1, "limits", "line byte limit exceeded"));
            }
            total = total
                .checked_add(consumed)
                .filter(|length| *length <= limits.max_total_bytes)
                .ok_or_else(|| error(count + 1, "limits", "total byte limit exceeded"))?;
            bytes.extend_from_slice(&available[..content]);
            reader.consume(consumed);
            if newline.is_some() {
                break;
            }
        }
        if !has_data {
            break;
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| error(count, "limits", "line counter overflow"))?;
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        let record = parse_record(&bytes, count)?;
        state.apply(&record)?;
        consumer(&record, &state).map_err(|cause| {
            error(
                count,
                record.event.kind(),
                format!("consumer failed: {cause}"),
            )
        })?;
    }
    if !state.is_finished() {
        return Err(error(
            count + 1,
            "eof",
            "missing end_game; input is incomplete",
        ));
    }
    Ok(state.summary().clone())
}

fn parse_record(bytes: &[u8], line: usize) -> Result<ParsedEvent, ApiError> {
    let raw: UniqueValue =
        serde_json::from_slice(bytes).map_err(|cause| error(line, "json", cause))?;
    let kind = raw
        .0
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let event = serde_json::from_value(raw.0.clone()).map_err(|cause| error(line, kind, cause))?;
    Ok(ParsedEvent {
        line,
        event,
        raw: raw.0,
    })
}

// A Value alone would lose duplicate object keys before serde sees the event.
// This visitor preserves the protocol's reject-on-duplicate contract recursively.
// Finite extension metadata may contain floats; typed event counters remain integers.
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
        formatter.write_str("JSON with unique object keys")
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
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
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
