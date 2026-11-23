//! Original JSON is the oracle; typed/float projections are never the expected output.
use serde_json::json;
use std::io::{self, BufRead, Cursor, Read, Write};
use suzume_api::{
    logging::LogLimits,
    mjai::{
        self,
        annotated::{self, AnnotationPolicy},
    },
    types::{ApiError, ErrorCode, SeatId},
};

const START: &str = r#"{"type": "start_game", "names":["A","B","C","D"], "extra":{"large":9007199254740993e0,"tiny":1e-400,"exponent":1e20,"integer":18446744073709551617,"decimal":0.12345678901234567890123456789,"null":null,"escaped":"a\u0020b"}}"#;
const END: &str = r#"{"type":"end_game"}"#;
const NOTES: &str =
    r#"{"review": {"value":9007199254740993e0,"tiny":1e-400,"text":"5mr\n評価","missing":null}}"#;

fn source() -> String {
    format!("{START}\n{END}\n")
}

// Independent fixture construction also permits invalid lexical tokens.
fn envelope(event: &str, index: &str, annotations: &str) -> String {
    format!(
        r#"{{"record":"annotated_mjai","schema_version":1,"index":{index},"event":{event},"annotations":{annotations}}}"#
    )
}

fn wrapped(notes: &str) -> String {
    format!(
        "{}\n{}\n",
        envelope(START, "1", notes),
        envelope(END, "2", "{}")
    )
}

#[test]
fn single_record_retains_original_json_with_precision_null_and_key_order() {
    let parsed = mjai::parse_record(&format!(" \t{START}\r\n")).unwrap();
    assert_eq!(parsed.json, START);
    assert_eq!(parsed.line, 1);
    assert_ne!(parsed.raw.to_string(), parsed.json);
    let event = r#"{ "type":"hora", "actor":0,"target":1,"deltas":null,"uradora_markers":["5mr"],"extra":{"b":2,"a":1}}"#;
    let record = annotated::parse_record(&envelope(event, "1", NOTES)).unwrap();
    assert_eq!(record.event.json, event);
    assert_eq!(record.annotations_json, NOTES);
    assert_eq!(record.annotations["review"]["text"], "5mr\n評価");
    assert!(record.event.raw.get("uradora_markers").is_some());
    assert!(record.event.raw.get("ura_markers").is_none());
}

#[test]
fn wrap_and_unwrap_preserve_event_originals_and_summary() {
    let input = source();
    let mut encoded = Vec::new();
    let report =
        annotated::wrap(&mut Cursor::new(&input), &mut encoded, LogLimits::default()).unwrap();
    assert_eq!(report.summary.records, 2);
    assert_eq!(report.dropped_annotation_records, 0);
    let mut decoded = Vec::new();
    let unwrapped = annotated::unwrap(
        &mut Cursor::new(&encoded),
        &mut decoded,
        LogLimits::default(),
        AnnotationPolicy::Reject,
    )
    .unwrap();
    assert_eq!(decoded, input.as_bytes());
    assert_eq!(report.summary, unwrapped.summary);
    assert_eq!(
        report.summary,
        mjai::analyze(&mut Cursor::new(input), LogLimits::default()).unwrap()
    );
    assert_eq!(String::from_utf8(encoded).unwrap().lines().count(), 2);
}

#[test]
fn wrap_rejects_internal_cr_before_writing_an_unreadable_envelope() {
    // The legacy standard reader accepts JSON whitespace CR within a physical
    // line. The annotated format deliberately promises no internal CR or LF.
    let input = "{\"type\":\r\"start_game\"}\n{\"type\":\"end_game\"}\n";
    assert!(mjai::analyze(&mut Cursor::new(input), LogLimits::default()).is_ok());
    let mut output = Vec::new();
    let error =
        annotated::wrap(&mut Cursor::new(input), &mut output, LogLimits::default()).unwrap_err();
    assert!(error.message.contains("physical line"), "{error}");
    assert!(output.is_empty());
}

#[test]
fn raw_subtrees_preserve_the_existing_depth_and_finite_number_limits() {
    let event_at_depth = |depth: usize| {
        format!(
            "{{\"type\":\"start_game\",\"extra\":{}0{}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    let depth = (0..256)
        .take_while(|depth| mjai::parse_record(&event_at_depth(*depth)).is_ok())
        .last()
        .unwrap();
    assert!(depth > 1);
    let event = event_at_depth(depth);
    let encoded = annotated::encode_record(&event, 1, "{}", LogLimits::default()).unwrap();
    assert_eq!(annotated::parse_record(&encoded).unwrap().event.json, event);
    assert!(annotated::parse_record(&envelope(&event_at_depth(depth + 1), "1", "{}")).is_err());
    let annotations = format!("{{\"nested\":{}0{}}}", "[".repeat(256), "]".repeat(256));
    assert!(annotated::parse_record(&envelope(START, "1", &annotations)).is_err());
    for (event, annotations) in [
        (r#"{"type":"start_game","extra":1e400}"#, "{}"),
        (START, r#"{"extra":1e400}"#),
    ] {
        assert!(annotated::parse_record(&envelope(event, "1", annotations)).is_err());
        assert!(annotated::encode_record(event, 1, annotations, LogLimits::default()).is_err());
    }
}

#[test]
fn encoder_preserves_both_originals_without_trusting_mutated_projections() {
    let mut record = annotated::parse_record(&envelope(START, "1", NOTES)).unwrap();
    record.event.raw["extra"] = json!(null);
    record.annotations.clear();
    let encoded = annotated::encode_record(
        &record.event.json,
        record.index,
        &record.annotations_json,
        LogLimits::default(),
    )
    .unwrap();
    assert!(!encoded.contains(['\r', '\n']));
    let decoded = annotated::parse_record(&encoded).unwrap();
    assert_eq!(decoded.event.json, START);
    assert_eq!(decoded.annotations_json, NOTES);
}

#[test]
fn annotations_require_explicit_drop_and_count_annotated_records() {
    let input = wrapped(NOTES);
    let error = annotated::unwrap(
        &mut Cursor::new(&input),
        &mut Vec::new(),
        LogLimits::default(),
        AnnotationPolicy::Reject,
    )
    .unwrap_err();
    assert!(error.message.contains("annotation"), "{error}");
    let mut output = Vec::new();
    let report = annotated::unwrap(
        &mut Cursor::new(&input),
        &mut output,
        LogLimits::default(),
        AnnotationPolicy::Drop,
    )
    .unwrap();
    assert_eq!(report.dropped_annotation_records, 1);
    assert_eq!(output, source().as_bytes());
    let mut observed = 0;
    annotated::visit_records(
        &mut Cursor::new(input),
        LogLimits::default(),
        |record, state| {
            assert_eq!(record.index, record.event.line);
            assert_eq!(record.index as u32, state.summary().records);
            if record.index == 1 {
                assert_eq!(record.annotations_json, NOTES);
            }
            observed += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(observed, 2);
}

#[test]
fn rejects_bad_outer_contract_duplicates_and_integer_lexemes() {
    let valid = envelope(START, "1", "{}");
    let cases = [
        valid.replace("annotated_mjai", "other"),
        valid.replace("\"schema_version\":1", "\"schema_version\":2"),
        valid.replace("\"schema_version\":1", "\"schema_version\":1.0"),
        valid.replace("\"schema_version\":1", "\"schema_version\":1e0"),
        valid.replace("\"index\":1", "\"index\":0"),
        valid.replace("\"index\":1", "\"index\":-1"),
        valid.replace("\"index\":1", "\"index\":1e0"),
        valid.replace("\"index\":1", "\"index\":4294967296"),
        valid.replace("\"index\":1,", ""),
        valid.replace("\"index\":1,", "\"index\":1,\"index\":1,"),
        valid.replacen('{', "{\"unknown\":true,", 1),
        envelope(START, "1", "null"),
        envelope(START, "1", "[]"),
        envelope(START, "1", r#"{"a":{"duplicate":1,"duplicate":2}}"#),
        envelope(r#"{"type":"start_game","extra":{"a":1,"a":2}}"#, "1", "{}"),
        envelope(r#"{"type":"tsumo","actor":1.0,"pai":"?"}"#, "1", "{}"),
        envelope(r#"{"type":"tsumo","actor":1e0,"pai":"?"}"#, "1", "{}"),
        envelope(r#"{"type":"tsumo","actor":-0,"pai":"?"}"#, "1", "{}"),
        envelope(r#"{"type":"future_event"}"#, "1", "{}"),
    ];
    for (case, input) in cases.iter().enumerate() {
        assert!(annotated::parse_record(input).is_err(), "case {case}");
    }
    assert_eq!(
        annotated::parse_record(&envelope(START, "4294967295", "{}"))
            .unwrap()
            .index,
        u32::MAX as usize
    );
}

#[test]
fn encoder_enforces_index_single_physical_line_and_json_ascii_whitespace() {
    assert!(annotated::encode_record(START, 0, "{}", LogLimits::default()).is_err());
    if let Some(index) = (u32::MAX as usize).checked_add(1) {
        assert!(annotated::encode_record(START, index, "{}", LogLimits::default()).is_err());
    }
    for event in [
        "{\n\"type\":\"start_game\"}",
        "{\r\"type\":\"start_game\"}",
        "\u{a0}{\"type\":\"start_game\"}",
    ] {
        assert!(annotated::encode_record(event, 1, "{}", LogLimits::default()).is_err());
    }
    assert!(annotated::encode_record(START, 1, "{\n\"a\":1}", LogLimits::default()).is_err());
    assert!(annotated::encode_record(START, 1, "\u{a0}{}", LogLimits::default()).is_err());
    let encoded = annotated::encode_record(
        &format!(" \t{START}\r\n"),
        1,
        &format!("\t{NOTES}\r\n"),
        LogLimits::default(),
    )
    .unwrap();
    assert_eq!(
        annotated::parse_record(&encoded).unwrap().annotations_json,
        NOTES
    );
}

#[test]
fn visits_validate_contiguous_indexes_end_game_and_semantics() {
    for input in [
        format!(
            "{}\n{}\n",
            envelope(START, "2", "{}"),
            envelope(END, "3", "{}")
        ),
        format!(
            "{}\n{}\n",
            envelope(START, "1", "{}"),
            envelope(END, "1", "{}")
        ),
        format!(
            "{}\n{}\n",
            envelope(START, "1", "{}"),
            envelope(END, "3", "{}")
        ),
        format!("{}\n", envelope(START, "1", "{}")),
        format!(
            "{}{}\n",
            wrapped("{}"),
            envelope(r#"{"type":"none"}"#, "3", "{}")
        ),
    ] {
        assert!(
            annotated::visit(&mut Cursor::new(input), LogLimits::default(), |_, _| Ok(())).is_err()
        );
    }
    let input = format!(
        "{}\n{}\n",
        envelope(START, "1", "{}"),
        envelope(
            r#"{"type":"dahai","actor":0,"pai":"1m","tsumogiri":true}"#,
            "2",
            "{}"
        )
    );
    let mut seen = 0;
    let error = annotated::visit(&mut Cursor::new(input), LogLimits::default(), |_, _| {
        seen += 1;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(seen, 1);
    assert!(error.message.contains("line 2"), "{error}");
    assert!(error.message.contains("dahai"), "{error}");
}

#[test]
fn output_line_and_total_limits_count_utf8_and_final_lf() {
    let event = r#"{"type":"start_game","names":["あ","B","C","D"]}"#;
    let encoded = annotated::encode_record(event, 1, "{}", LogLimits::default()).unwrap();
    let boundary = LogLimits {
        max_line_bytes: encoded.len(),
        max_total_bytes: encoded.len(),
        ..LogLimits::default()
    };
    assert_eq!(
        annotated::encode_record(event, 1, "{}", boundary).unwrap(),
        encoded
    );
    for limits in [
        LogLimits {
            max_line_bytes: encoded.len() - 1,
            ..boundary
        },
        LogLimits {
            max_line_bytes: encoded.chars().count(),
            ..boundary
        },
        LogLimits {
            max_total_bytes: encoded.len() - 1,
            ..boundary
        },
        LogLimits {
            max_records: 0,
            ..boundary
        },
    ] {
        assert!(annotated::encode_record(event, 1, "{}", limits).is_err());
    }
    let input = source();
    let expected = format!(
        "{}\n{}\n",
        envelope(START, "1", "{}"),
        envelope(END, "2", "{}")
    );
    let limits = LogLimits {
        max_total_bytes: expected.len(),
        ..LogLimits::default()
    };
    let mut output = Vec::new();
    annotated::wrap(&mut Cursor::new(&input), &mut output, limits).unwrap();
    assert_eq!(output.len(), expected.len());
    let error = annotated::wrap(
        &mut Cursor::new(&input),
        &mut Vec::new(),
        LogLimits {
            max_total_bytes: expected.len() - 1,
            ..limits
        },
    )
    .unwrap_err();
    assert!(error.message.contains("output"), "{error}");
    let error = annotated::wrap(
        &mut Cursor::new(input),
        &mut Vec::new(),
        LogLimits {
            max_line_bytes: START.len(),
            ..LogLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("output"), "{error}");
}

#[test]
fn annotated_input_limits_cover_crlf_records_and_invalid_utf8() {
    let input = wrapped("{}");
    let max_line = input.lines().map(str::len).max().unwrap();
    for limits in [
        LogLimits {
            max_records: 1,
            ..LogLimits::default()
        },
        LogLimits {
            max_total_bytes: input.len() - 1,
            ..LogLimits::default()
        },
        LogLimits {
            max_line_bytes: max_line - 1,
            ..LogLimits::default()
        },
        LogLimits {
            max_line_bytes: 0,
            ..LogLimits::default()
        },
    ] {
        assert!(annotated::visit(&mut Cursor::new(&input), limits, |_, _| Ok(())).is_err());
    }
    let crlf = input.trim_end_matches('\n').replace('\n', "\r\n");
    assert!(annotated::visit(
        &mut Cursor::new(&crlf),
        LogLimits {
            max_line_bytes: max_line + 1,
            ..LogLimits::default()
        },
        |_, _| Ok(())
    )
    .is_ok());
    assert!(annotated::visit(
        &mut Cursor::new(crlf),
        LogLimits {
            max_line_bytes: max_line,
            ..LogLimits::default()
        },
        |_, _| Ok(())
    )
    .is_err());
    assert!(annotated::visit(
        &mut Cursor::new([0xff, b'\n']),
        LogLimits::default(),
        |_, _| Ok(())
    )
    .is_err());
    assert!(annotated::visit(&mut Cursor::new("\n"), LogLimits::default(), |_, _| Ok(())).is_err());
}

#[test]
fn wrappers_do_not_change_replay_observations_or_count_annotation_red_tiles() {
    let values = [
        json!({"type":"start_game","names":["5mr","B","C","D"]}),
        json!({"type":"start_kyoku","bakaze":"E","dora_marker":"4p","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"scores":[25000,25000,25000,25000],"tehais":[vec!["?";13],vec!["?";13],vec!["?";13],vec!["?";13]]}),
        json!({"type":"tsumo","actor":0,"pai":"5mr"}),
        json!({"type":"dahai","actor":0,"pai":"5mr","tsumogiri":true}),
        json!({"type":"ryukyoku","deltas":[0,0,0,0]}),
        json!({"type":"end_kyoku"}),
        json!({"type":"end_game"}),
    ];
    let input: String = values.iter().map(|value| format!("{value}\n")).collect();
    let annotated_input: String = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            format!(
                "{}\n",
                envelope(
                    &value.to_string(),
                    &(index + 1).to_string(),
                    r#"{"private_note":"5sr"}"#
                )
            )
        })
        .collect();
    let mut observations = Vec::new();
    let summary = mjai::visit(&mut Cursor::new(input), LogLimits::default(), |_, state| {
        observations.push(std::array::from_fn::<_, 4, _>(|seat| {
            state.observe(SeatId::new(seat as u8))
        }));
        Ok(())
    })
    .unwrap();
    let mut index = 0;
    let annotated_summary = annotated::visit(
        &mut Cursor::new(annotated_input),
        LogLimits::default(),
        |record, state| {
            assert_eq!(record.line, index + 1);
            let actual =
                std::array::from_fn::<_, 4, _>(|seat| state.observe(SeatId::new(seat as u8)));
            assert_eq!(actual, observations[index]);
            assert!(!serde_json::to_string(&actual)
                .unwrap()
                .contains("private_note"));
            index += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(summary, annotated_summary);
    assert_eq!(summary.red_events, 2);
    assert_eq!(index, values.len());
}

struct BrokenTail(Cursor<Vec<u8>>);
impl Read for BrokenTail {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read tail failure"))
    }
}
impl BufRead for BrokenTail {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.0.position() as usize == self.0.get_ref().len() {
            Err(io::Error::other("read tail failure"))
        } else {
            self.0.fill_buf()
        }
    }
    fn consume(&mut self, amount: usize) {
        self.0.consume(amount);
    }
}
struct BrokenWrite;
impl Write for BrokenWrite {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("write failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn conversions_never_report_success_before_clean_eof_and_successful_writes() {
    let error = annotated::wrap(
        &mut BrokenTail(Cursor::new(source().into_bytes())),
        &mut Vec::new(),
        LogLimits::default(),
    )
    .unwrap_err();
    assert!(error.message.contains("read tail failure"), "{error}");
    let error = annotated::unwrap(
        &mut BrokenTail(Cursor::new(wrapped("{}").into_bytes())),
        &mut Vec::new(),
        LogLimits::default(),
        AnnotationPolicy::Reject,
    )
    .unwrap_err();
    assert!(error.message.contains("read tail failure"), "{error}");
    let error = annotated::wrap(
        &mut Cursor::new(source()),
        &mut BrokenWrite,
        LogLimits::default(),
    )
    .unwrap_err();
    assert!(error.message.contains("write failure"), "{error}");
    let error = annotated::unwrap(
        &mut Cursor::new(wrapped("{}")),
        &mut BrokenWrite,
        LogLimits::default(),
        AnnotationPolicy::Reject,
    )
    .unwrap_err();
    assert!(error.message.contains("write failure"), "{error}");
    let error = annotated::visit(
        &mut Cursor::new(wrapped("{}")),
        LogLimits::default(),
        |_, _| Err(ApiError::new(ErrorCode::LogError, "consumer failure")),
    )
    .unwrap_err();
    assert!(error.message.contains("line 1"), "{error}");
    assert!(error.message.contains("consumer failure"), "{error}");
    assert!(annotated::wrap(
        &mut Cursor::new(format!("{START}\n")),
        &mut Vec::new(),
        LogLimits::default()
    )
    .is_err());
    assert!(annotated::unwrap(
        &mut Cursor::new(format!("{}\n", envelope(START, "1", "{}"))),
        &mut Vec::new(),
        LogLimits::default(),
        AnnotationPolicy::Reject
    )
    .is_err());
}
