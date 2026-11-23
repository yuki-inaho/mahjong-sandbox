//! Versioned annotations around original mjai JSON, without reserializing events.
//!
//! Raw full-information records are archival data, not a player's observation.
//! Every streamed result is provisional until a valid `end_game` and clean EOF.
//! Low-level writers may contain a prefix after any error; publish files only
//! after conversion succeeds and the caller flushes/synchronizes its output.
use super::{error, reader, ParsedEvent, ReplayState, Summary};
use crate::{logging::LogLimits, types::ApiError};
use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Map, Value};
use std::io::{BufRead, Write};

#[derive(Clone, Debug)]
pub struct AnnotatedRecord {
    /// Positive u32 on the wire; stream visitors also require contiguous indexes.
    pub index: usize,
    pub event: ParsedEvent,
    /// Inspection projection. Numbers may be approximate; save annotations_json.
    pub annotations: Map<String, Value>,
    /// Validated original annotation object, excluding surrounding whitespace.
    pub annotations_json: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnnotationPolicy {
    #[default]
    Reject,
    Drop,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TranscodeReport {
    pub summary: Summary,
    pub dropped_annotation_records: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<'a> {
    record: String,
    schema_version: u32,
    index: u32,
    #[serde(borrow)]
    event: &'a RawValue,
    #[serde(borrow)]
    annotations: &'a RawValue,
}

/// Validate a single physical line with default limits, retaining both originals.
/// This does not claim that the event is valid at this index in an actual game.
pub fn parse_record(text: &str) -> Result<AnnotatedRecord, ApiError> {
    if text.len() > LogLimits::default().max_line_bytes {
        return Err(error(1, "annotated_mjai", "line byte limit exceeded"));
    }
    parse_record_at(text.as_bytes(), 1)
}

fn parse_record_at(bytes: &[u8], line: usize) -> Result<AnnotatedRecord, ApiError> {
    let text = std::str::from_utf8(bytes).map_err(|cause| error(line, "annotated_mjai", cause))?;
    let text = reader::single_line_fragment(text, line, "annotated_mjai")?;
    let envelope: Envelope<'_> =
        serde_json::from_str(text).map_err(|cause| error(line, "annotated_mjai", cause))?;
    if envelope.record != "annotated_mjai" {
        return Err(error(line, "annotated_mjai", "unsupported record kind"));
    }
    if envelope.schema_version != 1 {
        return Err(error(line, "annotated_mjai", "unsupported schema_version"));
    }
    if envelope.index == 0 {
        return Err(error(
            line,
            "annotated_mjai",
            "index must be a positive u32",
        ));
    }
    // Parse the original subtree, never its Value reserialization. This keeps
    // integer lexemes and the existing event recursion/number limits intact.
    let event = reader::parse_record_at(envelope.event.get().as_bytes(), line)?;
    let annotations_json = envelope.annotations.get();
    let annotations = parse_annotations(annotations_json, line)?;
    Ok(AnnotatedRecord {
        index: envelope.index as usize,
        event,
        annotations,
        annotations_json: annotations_json.to_owned(),
    })
}

fn parse_annotations(text: &str, line: usize) -> Result<Map<String, Value>, ApiError> {
    match reader::parse_unique_json(text.as_bytes(), line, "annotations")? {
        Value::Object(object) => Ok(object),
        _ => Err(error(line, "annotations", "annotations must be an object")),
    }
}

/// Encode a structurally valid record, without an LF. Input strings, not mutable
/// parsed projections, are revalidated. Limits count the returned UTF-8 bytes;
/// streamed conversions additionally charge their LF to the total byte budget.
pub fn encode_record(
    event_json: &str,
    index: usize,
    annotations_json: &str,
    limits: LogLimits,
) -> Result<String, ApiError> {
    reader::validate_limits(limits)?;
    validate_index(index)?;
    for text in [event_json, annotations_json] {
        if text.len() > limits.max_line_bytes {
            return Err(error(index, "limits", "input line byte limit exceeded"));
        }
    }
    let event_json = reader::single_line_fragment(event_json, index, "json")?;
    let annotations_json = reader::single_line_fragment(annotations_json, index, "annotations")?;
    reader::parse_record_at(event_json.as_bytes(), index)?;
    parse_annotations(annotations_json, index)?;
    encode_fragments(event_json, index, annotations_json, limits)
}

fn validate_index(index: usize) -> Result<(), ApiError> {
    if index == 0 || u32::try_from(index).is_err() {
        return Err(error(
            index,
            "annotated_mjai",
            "index must be a positive u32",
        ));
    }
    Ok(())
}

/// Only called with validated JSON originals. The standard reader can accept
/// internal CR whitespace, so enforce the annotated format's physical-line
/// contract here too. Source tokens are embedded as objects without Value output.
fn encode_fragments(
    event_json: &str,
    index: usize,
    annotations_json: &str,
    limits: LogLimits,
) -> Result<String, ApiError> {
    validate_index(index)?;
    let event_json = reader::single_line_fragment(event_json, index, "json")?;
    let annotations_json = reader::single_line_fragment(annotations_json, index, "annotations")?;
    let mut text = format!(
        "{{\"record\":\"annotated_mjai\",\"schema_version\":1,\"index\":{index},\"event\":"
    );
    let length = text
        .len()
        .checked_add(event_json.len())
        .and_then(|value| value.checked_add(",\"annotations\":".len()))
        .and_then(|value| value.checked_add(annotations_json.len()))
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| error(index, "output", "output byte count overflow"))?;
    if length > limits.max_line_bytes {
        return Err(error(index, "output", "output line byte limit exceeded"));
    }
    if length > limits.max_total_bytes {
        return Err(error(index, "output", "output total byte limit exceeded"));
    }
    text.reserve(length - text.len());
    text.push_str(event_json);
    text.push_str(",\"annotations\":");
    text.push_str(annotations_json);
    text.push('}');
    Ok(text)
}

/// Visit the same validated event/state pair as standard `mjai::visit`.
/// Annotations never enter the replay state, statistics or player observations.
pub fn visit<R: BufRead + ?Sized, F>(
    reader: &mut R,
    limits: LogLimits,
    mut consumer: F,
) -> Result<Summary, ApiError>
where
    F: FnMut(&ParsedEvent, &ReplayState) -> Result<(), ApiError>,
{
    visit_records(reader, limits, |record, state| {
        consumer(&record.event, state)
    })
}

/// Inspect original annotations alongside each accepted replay prefix.
/// Any later error invalidates the stream; success requires a clean input EOF.
pub fn visit_records<R: BufRead + ?Sized, F>(
    reader: &mut R,
    limits: LogLimits,
    mut consumer: F,
) -> Result<Summary, ApiError>
where
    F: FnMut(&AnnotatedRecord, &ReplayState) -> Result<(), ApiError>,
{
    let mut state = ReplayState::new();
    let count = reader::visit_lines(reader, limits, |bytes, line| {
        let record = parse_record_at(bytes, line)?;
        if record.index != line {
            return Err(error(
                line,
                "annotated_mjai",
                "index must be contiguous starting at 1",
            ));
        }
        state.apply(&record.event)?;
        consumer(&record, &state).map_err(|cause| {
            error(
                line,
                record.event.event.kind(),
                format!("consumer failed: {cause}"),
            )
        })
    })?;
    reader::finish(&state, count)
}

/// Wrap a complete standard stream with empty annotations, retaining event JSON.
pub fn wrap<R: BufRead + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    writer: &mut W,
    limits: LogLimits,
) -> Result<TranscodeReport, ApiError> {
    let mut output = LineOutput::new(writer, limits)?;
    let summary = super::visit(reader, limits, |record, _| {
        let text = encode_fragments(&record.json, record.line, "{}", limits)?;
        output.write_line(&text, record.line)
    })?;
    Ok(TranscodeReport {
        summary,
        dropped_annotation_records: 0,
    })
}

/// Export standard mjai. Nonempty annotations require an explicit Drop policy.
/// The annotation source is not changed, and discarded annotations are not
/// recoverable by wrapping the standard output again.
pub fn unwrap<R: BufRead + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    writer: &mut W,
    limits: LogLimits,
    policy: AnnotationPolicy,
) -> Result<TranscodeReport, ApiError> {
    let mut output = LineOutput::new(writer, limits)?;
    let mut dropped_annotation_records = 0u32;
    let summary = visit_records(reader, limits, |record, _| {
        if !record.annotations.is_empty() {
            if policy == AnnotationPolicy::Reject {
                return Err(error(
                    record.event.line,
                    "annotations",
                    "nonempty annotations require explicit Drop policy",
                ));
            }
            dropped_annotation_records =
                dropped_annotation_records.checked_add(1).ok_or_else(|| {
                    error(
                        record.event.line,
                        "annotations",
                        "annotation count overflow",
                    )
                })?;
        }
        output.write_line(&record.event.json, record.event.line)
    })?;
    Ok(TranscodeReport {
        summary,
        dropped_annotation_records,
    })
}

struct LineOutput<'a, W: Write + ?Sized> {
    writer: &'a mut W,
    limits: LogLimits,
    records: usize,
    total: usize,
}

impl<'a, W: Write + ?Sized> LineOutput<'a, W> {
    fn new(writer: &'a mut W, limits: LogLimits) -> Result<Self, ApiError> {
        reader::validate_limits(limits)?;
        Ok(Self {
            writer,
            limits,
            records: 0,
            total: 0,
        })
    }

    fn write_line(&mut self, text: &str, line: usize) -> Result<(), ApiError> {
        if text.len() > self.limits.max_line_bytes {
            return Err(error(line, "output", "output line byte limit exceeded"));
        }
        if self.records >= self.limits.max_records {
            return Err(error(line, "output", "output record count exceeds limit"));
        }
        let total = self
            .total
            .checked_add(text.len())
            .and_then(|value| value.checked_add(1))
            .filter(|value| *value <= self.limits.max_total_bytes)
            .ok_or_else(|| error(line, "output", "output total byte limit exceeded"))?;
        self.writer
            .write_all(text.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .map_err(|cause| error(line, "output", cause))?;
        self.total = total;
        self.records += 1;
        Ok(())
    }
}
