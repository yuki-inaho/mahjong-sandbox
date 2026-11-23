use std::cell::Cell;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::rc::Rc;

use serde_json::{json, Value};
use suzume::game::{Action, Game, Phase};
use suzume_api::config::{
    AiConfigSpec, Algorithm, AppConfig, Controller, LogFormat, PublicViewer, Viewer,
};
use suzume_api::decision::DecisionView;
use suzume_api::environment::Environment;
use suzume_api::logging::{
    export_transitions, replay, DatasetTransition, EndReason, LogLimits, LogRecord, LogWriter,
};
use suzume_api::types::{ActionId, ApiError, ErrorCode, Seed};

fn fixture(seed: u64, steps: u32, format: LogFormat) -> (Vec<u8>, Game) {
    let mut config = AppConfig::default();
    config.game.seed = Seed::from(seed);
    config.game.max_steps = steps;
    config.logging.format = format;
    let mut game = Game::new(seed).unwrap();
    let mut writer = LogWriter::new(Vec::new(), &config, &game).unwrap();
    for _ in 0..steps {
        let actor = game.next_actor().unwrap();
        let action = game.legal_actions(actor).remove(0);
        game.apply(actor, action.clone()).unwrap();
        writer.record(actor, &action, &game, None).unwrap();
    }
    let bytes = writer.finish(&game, EndReason::Truncated).unwrap();
    (bytes, game)
}

fn values(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn encode(values: &[Value]) -> Vec<u8> {
    let mut result = Vec::new();
    for value in values {
        serde_json::to_writer(&mut result, value).unwrap();
        result.push(b'\n');
    }
    result
}

fn rejects(bytes: &[u8]) -> ApiError {
    let error = replay(&mut Cursor::new(bytes), LogLimits::default()).unwrap_err();
    assert_eq!(error.code, ErrorCode::LogError);
    error
}

#[test]
fn replay_restores_full_game_and_exact_hand_index_history() {
    for seed in [0, 9, u64::MAX] {
        let (bytes, original) = fixture(seed, 80, LogFormat::Replay);
        let result = replay(&mut Cursor::new(bytes), LogLimits::default()).unwrap();
        assert_eq!(result.metadata.seed.value(), seed);
        assert_eq!(result.termination.reason, EndReason::Truncated);
        assert_eq!(result.termination.action_count, 80);
        assert_eq!(result.game.history(), original.history());
        assert_eq!(result.game, original);
    }
}

#[test]
fn full_log_records_resolved_configuration_and_roundtrips_schema_types() {
    let (bytes, _) = fixture(42, 2, LogFormat::Replay);
    let rows = values(&bytes);
    assert_eq!(rows[0]["seed"], "42");
    assert_eq!(rows[0]["config"]["game"]["seed"], "42");
    assert_eq!(rows[0]["effective_ai"][1]["algorithm"], "greedy");
    assert_eq!(rows[0]["effective_ai"][1]["max_evaluations"], 1024);
    for row in rows {
        let typed: LogRecord = serde_json::from_value(row.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), row);
    }
}

#[test]
fn malformed_versions_metadata_fields_and_sequences_are_rejected() {
    let (bytes, _) = fixture(4, 3, LogFormat::Replay);
    let baseline = values(&bytes);
    let mutations: &[(usize, &str, Value)] = &[
        (0, "version", json!(2)),
        (0, "engine", json!("0.0.0")),
        (0, "rules_sha256", json!("0".repeat(64))),
        (0, "shuffle", json!("unknown")),
        (0, "seed", json!(4)),
        (0, "seed", json!("04")),
        (0, "seed", json!("18446744073709551616")),
        (0, "unrecognized", json!(true)),
        (1, "sequence", json!(2)),
        (1, "revision", json!(0)),
        (1, "actor", json!(4)),
        (1, "before_sha256", json!("0".repeat(64))),
        (1, "after_sha256", json!("0".repeat(64))),
        (1, "action", json!({"kind":"discard","index":14})),
        (
            1,
            "action",
            json!({"kind":"discard","index":0,"unknown":true}),
        ),
        (4, "action_count", json!(2)),
        (4, "revision", json!(2)),
        (4, "scores", json!([25001, 25000, 25000, 25000])),
        (4, "final_position_sha256", json!("0".repeat(64))),
        (4, "reason", json!("completed")),
    ];
    for (line, field, value) in mutations {
        let mut changed = baseline.clone();
        changed[*line][*field] = value.clone();
        rejects(&encode(&changed));
    }
    let mut changed = baseline.clone();
    changed[0]["effective_ai"][1]["max_evaluations"] = json!(1025);
    rejects(&encode(&changed));
    changed = baseline.clone();
    changed[0]["config"]["game"]["unknown"] = json!(0);
    rejects(&encode(&changed));
    changed = baseline;
    changed[1]["action"] = json!({"kind":"discard","index":13});
    rejects(&encode(&changed));
}

#[test]
fn duplicate_keys_at_top_level_and_inside_nested_types_are_rejected() {
    let (bytes, _) = fixture(4, 1, LogFormat::Replay);
    let text = String::from_utf8(bytes).unwrap();
    let duplicate_record = text.replacen(
        "\"record\":\"replay_header\"",
        "\"record\":\"replay_header\",\"record\":\"replay_header\"",
        1,
    );
    assert_ne!(text, duplicate_record);
    rejects(duplicate_record.as_bytes());
    let duplicate_seed = text.replacen("\"seed\":\"4\"", "\"seed\":\"4\",\"seed\":\"4\"", 1);
    assert_ne!(text, duplicate_seed);
    rejects(duplicate_seed.as_bytes());
    let duplicate_nested = text.replacen("\"max_steps\":1", "\"max_steps\":1,\"max_steps\":1", 1);
    assert_ne!(text, duplicate_nested);
    rejects(duplicate_nested.as_bytes());
}

#[test]
fn missing_reordered_extra_or_truncated_records_never_count_as_complete() {
    let (bytes, _) = fixture(8, 3, LogFormat::Replay);
    let rows = values(&bytes);
    for remove_at in [0, 1, 2, rows.len() - 1] {
        let mut changed = rows.clone();
        changed.remove(remove_at);
        rejects(&encode(&changed));
    }
    let mut changed = rows.clone();
    changed.swap(1, 2);
    rejects(&encode(&changed));
    changed = rows.clone();
    changed.insert(2, rows[1].clone());
    rejects(&encode(&changed));
    changed = rows.clone();
    changed.push(rows.last().unwrap().clone());
    rejects(&encode(&changed));
    rejects(&bytes[..bytes.len() - 8]);
    let mut extra = bytes.clone();
    extra.extend_from_slice(b"\n");
    rejects(&extra);
    let mut malformed = bytes;
    malformed[0] = 0xff;
    rejects(&malformed);
}

#[test]
fn line_count_total_and_buffer_limits_are_enforced() {
    let (bytes, _) = fixture(1, 3, LogFormat::Replay);
    let limits = [
        LogLimits {
            max_line_bytes: 32,
            ..LogLimits::default()
        },
        LogLimits {
            max_records: 4,
            ..LogLimits::default()
        },
        LogLimits {
            max_total_bytes: bytes.len() - 1,
            ..LogLimits::default()
        },
        LogLimits {
            max_line_bytes: 0,
            ..LogLimits::default()
        },
    ];
    for limit in limits {
        assert!(replay(&mut Cursor::new(&bytes), limit).is_err());
    }
    let mut reader = io::BufReader::with_capacity(7, Cursor::new(vec![b'x'; 100_000]));
    let limit = LogLimits {
        max_line_bytes: 32,
        ..LogLimits::default()
    };
    assert!(replay(&mut reader, limit).is_err());
    assert!(
        reader.get_ref().position() <= 42,
        "must reject before reading the full oversized line"
    );
}

fn assert_public(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    ![
                        "seed",
                        "index",
                        "decision",
                        "before_sha256",
                        "after_sha256",
                        "initial_position_sha256",
                        "final_position_sha256"
                    ]
                    .contains(&key.as_str()),
                    "private key {key}"
                );
                if ["own_hand", "own_last_draw", "own_furiten", "viewer"].contains(&key.as_str()) {
                    assert!(value.is_null());
                }
                if key == "legal_actions" {
                    assert_eq!(value, &json!([]));
                }
                assert_public(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_public(value);
            }
        }
        _ => {}
    }
}

#[test]
fn public_log_contains_only_public_views_and_semantic_actions() {
    let (bytes, _) = fixture(9_123_456_789, 12, LogFormat::Public);
    let rows = values(&bytes);
    assert_eq!(rows[0]["record"], "public_header");
    assert_eq!(rows[1]["action"]["kind"], "discard");
    assert!(rows[1]["action"]["tile"].is_number());
    assert_eq!(rows[2]["action"]["kind"], "response_submitted");
    for row in &rows {
        assert_public(row);
    }
    assert!(!String::from_utf8(bytes.clone())
        .unwrap()
        .contains("9123456789"));
    assert!(rejects(&bytes).message.contains("public"));
}

#[test]
fn writer_rejects_skips_wrong_actor_and_false_termination() {
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let mut writer = LogWriter::new(Vec::new(), &config, &game).unwrap();
    assert!(writer.record(0, &Action::Discard(0), &game, None).is_err());
    let action = game.legal_actions(0).remove(0);
    game.apply(0, action.clone()).unwrap();
    assert!(writer.record(usize::MAX, &action, &game, None).is_err());
    assert!(writer.record(1, &action, &game, None).is_err());
    assert!(writer.record(0, &Action::Pass, &game, None).is_err());
    writer.record(0, &action, &game, None).unwrap();
    assert!(writer.record(0, &action, &game, None).is_err());
    assert!(writer.finish(&game, EndReason::Completed).is_err());
    assert!(LogWriter::new(Vec::new(), &config, &game).is_err());
    let other = Game::new(1).unwrap();
    assert!(LogWriter::new(Vec::new(), &config, &other).is_err());
}

#[test]
fn interrupted_empty_and_nonempty_logs_are_explicitly_truncated_datasets() {
    for steps in [0, 2] {
        let config = AppConfig::default();
        let mut game = Game::new(0).unwrap();
        let mut writer = LogWriter::new(Vec::new(), &config, &game).unwrap();
        for _ in 0..steps {
            let actor = game.next_actor().unwrap();
            let action = game.legal_actions(actor).remove(0);
            game.apply(actor, action.clone()).unwrap();
            writer.record(actor, &action, &game, None).unwrap();
        }
        let bytes = writer.finish(&game, EndReason::Interrupted).unwrap();
        let mut rows = Vec::new();
        let result = export_transitions(&mut Cursor::new(bytes), LogLimits::default(), |row| {
            rows.push(row.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(result.game, game);
        assert_eq!(rows.len(), steps);
        if let Some(last) = rows.last() {
            assert!(last.truncated && !last.terminated);
            assert!(last.next_observation.is_none());
            assert!(last.next_actor.is_none());
            assert_eq!(
                last.final_observation,
                suzume_api::environment::EncodedObservation::for_actor(&game, 1, steps as u32)
            );
        }
    }
}

#[test]
fn dataset_uses_the_same_observations_actions_and_rewards_as_environment() {
    let mut config = AppConfig::default();
    config.game.seed = 27_u64.into();
    config.game.max_steps = 32;
    let mut env = Environment::new(&config.game.seed, 32, 1).unwrap();
    let mut writer = LogWriter::new(Vec::new(), &config, env.game()).unwrap();
    let mut expected = Vec::new();
    for _ in 0..32 {
        let before = env.state().observation.unwrap();
        let id = before.legal_mask.iter().position(|legal| *legal).unwrap();
        let transition = env.step(ActionId::new(id as u16).unwrap()).unwrap();
        let actor = usize::from(before.actor.get());
        let action = transition.action.as_ref().unwrap().to_core();
        writer.record(actor, &action, env.game(), None).unwrap();
        expected.push((before, transition));
    }
    let bytes = writer.finish(env.game(), EndReason::Truncated).unwrap();
    let mut rows: Vec<DatasetTransition> = Vec::new();
    let result = export_transitions(&mut Cursor::new(bytes), LogLimits::default(), |row| {
        rows.push(row.clone());
        Ok(())
    })
    .unwrap();
    assert_eq!(&result.game, env.game());
    assert_eq!(rows.len(), expected.len());
    for (row, (before, after)) in rows.iter().zip(expected) {
        assert_eq!(row.observation, before);
        assert_eq!(Some(row.actor), after.actor);
        assert_eq!(Some(row.action_id), after.action_id);
        assert_eq!(Some(&row.action), after.action.as_ref());
        assert!(row.observation.legal_mask[usize::from(row.action_id.get())]);
        assert_eq!(row.rewards, after.rewards);
        assert_eq!(row.escrow_delta, after.escrow_delta);
        assert_eq!(row.next_observation, after.observation);
        assert_eq!(row.final_observation, after.final_observation);
        assert_eq!(row.next_actor, after.observation.as_ref().map(|o| o.actor));
        assert_eq!(row.terminated, after.terminated);
        assert_eq!(row.truncated, after.truncated);
        assert_eq!(row.rewards.iter().sum::<i32>() + row.escrow_delta, 0);
    }
}

struct BrokenReader;
impl Read for BrokenReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read failure"))
    }
}
impl BufRead for BrokenReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Err(io::Error::other("read failure"))
    }
    fn consume(&mut self, _: usize) {}
}

struct FaultyWriter {
    remaining: Rc<Cell<usize>>,
    fail_flush: bool,
}
impl Write for FaultyWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.remaining.get());
        if count == 0 {
            return Err(io::Error::other("write failure"));
        }
        self.remaining.set(self.remaining.get() - count);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn io_and_callback_failures_are_not_successful_logs() {
    assert!(replay(&mut BrokenReader, LogLimits::default())
        .unwrap_err()
        .message
        .contains("read failure"));
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let remaining = Rc::new(Cell::new(0));
    assert!(LogWriter::new(
        FaultyWriter {
            remaining: remaining.clone(),
            fail_flush: false
        },
        &config,
        &game
    )
    .is_err());
    remaining.set(1_000_000);
    let mut writer = LogWriter::new(
        FaultyWriter {
            remaining: remaining.clone(),
            fail_flush: false,
        },
        &config,
        &game,
    )
    .unwrap();
    remaining.set(8);
    let action = game.legal_actions(0).remove(0);
    game.apply(0, action.clone()).unwrap();
    assert!(writer.record(0, &action, &game, None).is_err());
    remaining.set(1_000_000);
    assert!(
        writer.record(0, &action, &game, None).is_err(),
        "partial output poisons the writer"
    );
    let fresh = Game::new(0).unwrap();
    let writer = LogWriter::new(
        FaultyWriter {
            remaining,
            fail_flush: true,
        },
        &config,
        &fresh,
    )
    .unwrap();
    assert!(writer.finish(&fresh, EndReason::Interrupted).is_err());
    let (bytes, _) = fixture(0, 3, LogFormat::Replay);
    let error = export_transitions(&mut Cursor::new(bytes), LogLimits::default(), |_| {
        Err(ApiError::new(ErrorCode::LogError, "callback failure"))
    })
    .unwrap_err();
    assert_eq!(error.message, "callback failure");
}

#[test]
fn a_complete_match_is_distinct_from_an_external_cutoff() {
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let mut writer = LogWriter::new(Vec::new(), &config, &game).unwrap();
    while !matches!(game.phase(), Phase::MatchEnd) {
        assert!(game.history().len() < 10_000);
        let actor = game.next_actor().unwrap();
        let action = game.legal_actions(actor).remove(0);
        game.apply(actor, action.clone()).unwrap();
        writer.record(actor, &action, &game, None).unwrap();
    }
    let bytes = writer.finish(&game, EndReason::Completed).unwrap();
    let mut last = None;
    let replayed = export_transitions(&mut Cursor::new(bytes), LogLimits::default(), |row| {
        last = Some(row.clone());
        Ok(())
    })
    .unwrap();
    assert_eq!(replayed.game, game);
    let last = last.unwrap();
    assert!(last.terminated && !last.truncated);
    assert!(last.next_observation.is_none());
    assert!(last.final_observation.is_none());
}

#[test]
fn decision_metadata_is_consistent_and_never_appears_in_public_output() {
    let mut config = AppConfig::default();
    config.game.viewer = Viewer::Public(PublicViewer::Public);
    config.game.max_steps = 1;
    config.players = std::array::from_fn(|index| Controller::Ai {
        config: AiConfigSpec {
            algorithm: Some(Algorithm::Random),
            seed: (1_000_000 + index as u64).into(),
            ..Default::default()
        },
    });
    let mut game = Game::new(0).unwrap();
    let mut full = LogWriter::new(Vec::new(), &config, &game).unwrap();
    let mut public_config = config.clone();
    public_config.logging.format = LogFormat::Public;
    let mut public = LogWriter::new(Vec::new(), &public_config, &game).unwrap();
    let ai_config = match &config.players[0] {
        Controller::Ai { config } => config.resolve().unwrap(),
        Controller::Human => unreachable!(),
    };
    let ai = suzume::ai::ConfiguredAi::new(ai_config).unwrap();
    let decision = ai
        .decide(&game.observation(0).unwrap(), &game.legal_actions(0))
        .unwrap();
    let wire_decision = DecisionView::from_core(&decision).unwrap();
    let action = decision.action;
    game.apply(0, action.clone()).unwrap();
    let mut incorrect = wire_decision.clone();
    incorrect.evaluations = 1;
    assert!(full.record(0, &action, &game, Some(&incorrect)).is_err());
    full.record(0, &action, &game, Some(&wire_decision))
        .unwrap();
    public
        .record(0, &action, &game, Some(&wire_decision))
        .unwrap();
    let bytes = full.finish(&game, EndReason::Truncated).unwrap();
    let rows = values(&bytes);
    assert_eq!(rows[1]["decision"]["config"]["seed"], "1000000");
    assert_eq!(
        replay(&mut Cursor::new(&bytes), LogLimits::default())
            .unwrap()
            .game,
        game
    );
    let mut changed = rows.clone();
    changed[1]["decision"]["action"] = json!({"kind":"pass"});
    rejects(&encode(&changed));
    changed = rows.clone();
    changed[1]["decision"]["config"]["seed"] = json!("1000001");
    rejects(&encode(&changed));
    changed = rows;
    changed[1]["decision"]["evaluations"] = json!(1);
    rejects(&encode(&changed));
    let public_bytes = public.finish(&game, EndReason::Truncated).unwrap();
    for row in values(&public_bytes) {
        assert_public(&row);
    }
    assert!(!String::from_utf8(public_bytes).unwrap().contains("1000000"));
}

#[test]
fn complete_final_line_without_newline_and_crlf_are_accepted() {
    let (bytes, game) = fixture(0, 2, LogFormat::Replay);
    assert_eq!(bytes.last(), Some(&b'\n'));
    let no_newline = &bytes[..bytes.len() - 1];
    assert_eq!(
        replay(&mut Cursor::new(no_newline), LogLimits::default())
            .unwrap()
            .game,
        game
    );
    let crlf = String::from_utf8(bytes).unwrap().replace('\n', "\r\n");
    assert_eq!(
        replay(&mut Cursor::new(crlf), LogLimits::default())
            .unwrap()
            .game,
        game
    );
}

#[test]
fn streaming_output_is_provisional_until_the_footer_and_eof_are_validated() {
    let (bytes, _) = fixture(0, 3, LogFormat::Replay);
    let mut rows = values(&bytes);
    rows.last_mut().unwrap()["final_position_sha256"] = json!("0".repeat(64));
    let mut emitted = Vec::new();
    let error = export_transitions(
        &mut Cursor::new(encode(&rows)),
        LogLimits::default(),
        |row| {
            emitted.push(row.clone());
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::LogError);
    assert_eq!(emitted.len(), 2);
    assert!(emitted.iter().all(|row| !row.terminated && !row.truncated));
}

#[test]
fn truncated_logs_keep_the_last_observation_for_value_bootstrapping() {
    let (bytes, game) = fixture(0, 3, LogFormat::Replay);
    let mut rows = Vec::new();
    export_transitions(&mut Cursor::new(bytes), LogLimits::default(), |row| {
        rows.push(serde_json::to_value(row).unwrap());
        Ok(())
    })
    .unwrap();
    assert_eq!(rows[0]["final_observation"], Value::Null);
    let expected = suzume_api::environment::EncodedObservation::for_actor(&game, 1, 3).unwrap();
    assert_eq!(
        rows.last().unwrap()["final_observation"],
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(rows.last().unwrap()["next_observation"], Value::Null);
}
