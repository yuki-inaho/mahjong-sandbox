//! 外部プロセスとして起動し、表示モードと入力境界を検査します。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use suzume::ai::{ConfiguredAi, GreedyAi, Policy};
use suzume::game::{Game, Phase};
use suzume_api::config::{Algorithm, AppConfig, Controller};
use suzume_api::logging::{replay, EndReason, LogLimits, ReplayResult};

static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

struct TestFiles(PathBuf);

impl TestFiles {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "suzume-cli-test-{}-{}",
            std::process::id(),
            TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn config(&self, text: &str) -> PathBuf {
        let path = self.path("config.toml");
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TestFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn configured(path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_suzume"));
    command.arg("--config").arg(path).arg("--plain");
    command
}

fn read_replay(path: &Path) -> ReplayResult {
    let mut input = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    replay(&mut input, LogLimits::default()).unwrap()
}

fn all_ai_config(max_steps: u32) -> String {
    format!(
        "schema_version = 1\n\
         [logging]\nformat = \"replay\"\n\
         [game]\nseed = \"777\"\nviewer = \"public\"\nmax_steps = {max_steps}\n\
         [[players]]\nkind = \"ai\"\n[players.config]\npreset = \"beginner\"\nseed = \"11\"\n\
         [[players]]\nkind = \"ai\"\n[players.config]\nalgorithm = \"greedy\"\nseed = \"12\"\nmax_evaluations = 64\n\
         [[players]]\nkind = \"ai\"\n[players.config]\npreset = \"strong\"\nseed = \"13\"\nmax_evaluations = 100\nlookahead_depth = 1\nrisk_weight = 2\n\
         [[players]]\nkind = \"ai\"\n[players.config]\npreset = \"standard\"\nseed = \"14\"\n"
    )
}

#[test]
fn cli_config_applies_seed_to_snapshot() {
    let files = TestFiles::new();
    let config = files.config("schema_version = 1\n[game]\nseed = \"777\"\n");
    let configured_output = configured(&config).arg("--snapshot").output().unwrap();
    assert!(
        configured_output.status.success(),
        "{}",
        String::from_utf8_lossy(&configured_output.stderr)
    );
    let explicit_output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--seed", "777", "--snapshot", "--plain"])
        .output()
        .unwrap();
    assert_eq!(configured_output.stdout, explicit_output.stdout);
}

#[test]
fn cli_config_applies_the_operation_limit() {
    let files = TestFiles::new();
    let config = files.config(&all_ai_config(1));
    let output = configured(&config)
        .args(["--simulate", "1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("乱数種777"), "{text}");
    assert!(text.contains("上限1操作"), "{text}");
    assert!(text.contains("成功0 / 総数1、失敗1"), "{text}");
}

#[test]
fn cli_config_and_seed_are_rejected_as_conflicting() {
    let files = TestFiles::new();
    let config = files.config("schema_version = 1\n");
    let output = configured(&config).args(["--seed", "0"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("併用"));
}

#[test]
fn cli_config_rejects_incompatible_controllers() {
    let files = TestFiles::new();
    let automatic = files.config(&all_ai_config(10000));
    let output = configured(&automatic)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("席0"));
    let human = files.config("schema_version = 1\n");
    for mode in [vec!["--simulate", "1"], vec!["--demo"]] {
        let output = configured(&human).args(mode).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("全員AI"));
    }
}

#[test]
fn cli_config_reuses_shared_validation() {
    let files = TestFiles::new();
    for text in [
        "schema_version = 1\nunknown = 2\n".to_owned(),
        all_ai_config(1).replace(
            "preset = \"beginner\"",
            "preset = \"beginner\"\nalgorithm = \"random\"",
        ),
        all_ai_config(0),
    ] {
        let config = files.config(&text);
        let output = configured(&config)
            .args(["--simulate", "1"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("設定"));
    }
}

#[test]
fn cli_default_seed_is_reproducible() {
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_suzume"))
            .args(["--snapshot", "--plain"])
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(first.status.success() && second.status.success());
    assert!(
        first.stdout == second.stdout,
        "同じ既定条件の画面が一致しません"
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains("乱数種: 0"));
}

#[test]
fn cli_configured_policies_and_replay_match_actual_actions() {
    let files = TestFiles::new();
    let text = all_ai_config(40);
    let config_path = files.config(&text);
    let config = AppConfig::from_toml(&text).unwrap();
    let log_path = files.path("configured.jsonl");
    let output = configured(&config_path)
        .args(["--simulate", "1", "--log"])
        .arg(&log_path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let recorded = read_replay(&log_path);
    assert_eq!(recorded.metadata.config, config);
    assert_eq!(
        recorded.metadata.effective_ai,
        config.effective_ai().unwrap()
    );
    assert_eq!(recorded.termination.reason, EndReason::Truncated);
    assert_eq!(recorded.termination.action_count, 40);

    // Recompute each operation from the independently supplied TOML, rather
    // than trusting only the configuration copied into the log header.
    let mut expected = Game::new(777).unwrap();
    let mut seats_seen = [false; 4];
    let mut differs_from_old_greedy = false;
    for record in recorded.game.history() {
        let Controller::Ai { config: spec } = &config.players[record.player] else {
            panic!("全席AIの検査用設定です");
        };
        let policy = ConfiguredAi::new(spec.resolve().unwrap()).unwrap();
        let legal = expected.legal_actions(record.player);
        let observation = expected.observation(record.player).unwrap();
        let decision = policy.decide(&observation, &legal).unwrap();
        assert_eq!(record.action, decision.action);
        differs_from_old_greedy |=
            GreedyAi.choose_action(&observation, &legal) != Some(decision.action.clone());
        expected.apply(record.player, decision.action).unwrap();
        seats_seen[record.player] = true;
    }
    assert!(seats_seen.into_iter().all(|seen| seen));
    assert!(
        differs_from_old_greedy,
        "設定を無視した全員Greedyと区別できる操作列が必要です"
    );
    assert_eq!(expected, recorded.game);
    let log = std::fs::read_to_string(&log_path).unwrap();
    assert_eq!(log.matches("\"decision\":{").count(), 40);
}

#[test]
fn cli_complete_simulation_log_replays() {
    let files = TestFiles::new();
    let path = files.path("complete.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--simulate", "1", "--log-format", "replay", "--log"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recorded = read_replay(&path);
    assert_eq!(recorded.termination.reason, EndReason::Completed);
    assert!(matches!(recorded.game.phase(), Phase::MatchEnd));
    assert_eq!(recorded.metadata.seed.value(), 0);
    for config in recorded.metadata.effective_ai.into_iter().flatten() {
        assert_eq!(config.algorithm, Algorithm::Greedy);
        assert_eq!(config.seed.value(), 0);
    }
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("点数={:?}", recorded.game.scores())));
    assert!(text.contains("成功1 / 総数1、失敗0"));
}

#[test]
fn cli_public_log_omits_private_data() {
    let files = TestFiles::new();
    let config = files.config(&all_ai_config(8));
    let path = files.path("public.jsonl");
    let output = configured(&config)
        .args(["--simulate", "1", "--log"])
        .arg(&path)
        .args(["--log-format", "public"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let log = std::fs::read_to_string(&path).unwrap();
    assert!(log.contains("\"record\":\"public_header\""));
    assert!(log.contains("\"record\":\"public_footer\""));
    assert!(log.contains("\"reason\":\"truncated\""));
    assert!(log.contains("\"own_hand\":null"));
    for private_field in [
        "\"seed\":",
        "\"index\":",
        "\"decision\":",
        "\"own_hand\":[",
        "\"before_sha256\":",
        "\"initial_position_sha256\":",
    ] {
        assert!(!log.contains(private_field), "{private_field}");
    }
    let mut reader = std::io::Cursor::new(log.as_bytes());
    assert!(replay(&mut reader, LogLimits::default())
        .unwrap_err()
        .to_string()
        .contains("public"));
}

#[test]
fn cli_early_stops_write_valid_interrupted_logs() {
    for arguments in [vec!["--snapshot"], vec![]] {
        let files = TestFiles::new();
        let path = files.path("interrupted.jsonl");
        let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
            .args(arguments)
            .args(["--plain", "--log-format", "replay", "--log"])
            .arg(&path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let recorded = read_replay(&path);
        assert_eq!(recorded.termination.reason, EndReason::Interrupted);
        assert_eq!(recorded.termination.action_count, 0);
        assert_eq!(recorded.game, Game::new(0).unwrap());
    }
}

#[test]
fn cli_demo_stops_after_one_round_and_records_interruption() {
    let files = TestFiles::new();
    let config = files.config(&all_ai_config(10000));
    let path = files.path("demo.jsonl");
    let output = configured(&config)
        .args(["--demo", "--log"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recorded = read_replay(&path);
    assert_eq!(recorded.termination.reason, EndReason::Interrupted);
    assert!(matches!(recorded.game.phase(), Phase::RoundEnd(_)));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("公開情報のみ表示"));
    assert!(!text.contains("今回のツモ"));
    assert!(!text.contains("乱数種:"));
}

#[test]
fn cli_log_rejects_multiple_matches_and_existing_files() {
    let files = TestFiles::new();
    let path = files.path("keep.jsonl");
    std::fs::write(&path, "既存の記録").unwrap();
    for (count, code) in [("2", 2), ("1", 1)] {
        let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
            .args(["--simulate", count, "--log"])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "既存の記録");
    }
}

#[test]
fn cli_config_rejects_oversize_utf8_and_fixed_viewer_mismatch() {
    let files = TestFiles::new();
    let large = files.config(&" ".repeat(suzume_api::MAX_REQUEST_BYTES + 1));
    let output = configured(&large).arg("--snapshot").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("1MiB"));
    std::fs::write(&large, [0xff]).unwrap();
    let output = configured(&large).arg("--snapshot").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    let config = files.config(&all_ai_config(1).replace("viewer = \"public\"", "viewer = 1"));
    let output = configured(&config).arg("--demo").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("席0"));
}

#[test]
fn help_is_available_without_starting_a_game() {
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("--simulate"));
    assert!(text.contains("--snapshot"));
}

#[test]
fn invalid_cli_arguments_fail_with_a_reason() {
    for args in [
        vec!["--unknown"],
        vec!["--seed", "bad"],
        vec!["--simulate", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("入力エラー"));
    }
}

#[test]
fn plain_snapshot_has_no_terminal_control_sequences() {
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--snapshot", "--plain", "--seed", "0"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.contains(&0x1b));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("すずめ"));
    assert!(text.contains("伏せ牌13枚"));
    assert!(text.contains("東1局"));
    assert!(text.contains("乱数種: 0"));
}

#[test]
fn eof_closes_the_game_without_waiting() {
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--plain", "--seed", "0"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("入力が閉じられた"));
}

#[test]
fn invalid_human_input_recovers_and_quit_exits() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--plain", "--seed", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"invalid\nh\nq\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("入力を解釈できません"));
    assert!(text.contains("操作方法"));
    assert!(text.contains("対局を終了しました"));
}

#[test]
fn cli_default_log_is_mjson_and_matches_final_scores() {
    let files = TestFiles::new();
    let path = files.path("complete.mjson");
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--simulate", "1", "--log"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(matches!(
        suzume_api::mjai::parse_event(text.lines().next().unwrap()).unwrap(),
        suzume_api::mjai::Event::StartGame { .. }
    ));
    let summary =
        suzume_api::mjai::analyze(&mut std::io::Cursor::new(text), LogLimits::default()).unwrap();
    assert!(summary.complete);
    assert!(summary.rounds >= 4);
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains(&format!("点数={:?}", summary.final_scores.unwrap())));
}

#[test]
fn cli_mjai_public_masks_dealt_and_drawn_tiles() {
    let files = TestFiles::new();
    let path = files.path("public.mjson");
    let output = Command::new(env!("CARGO_BIN_EXE_suzume"))
        .args(["--snapshot", "--log-format", "mjai_public", "--log"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<_> = text
        .lines()
        .map(|line| suzume_api::mjai::parse_event(line).unwrap())
        .collect();
    let suzume_api::mjai::Event::StartKyoku { tehais, .. } = &rows[1] else {
        panic!("expected start_kyoku")
    };
    assert!(tehais.iter().flatten().all(|pai| pai.is_unknown()));
    let suzume_api::mjai::Event::Tsumo { pai, .. } = &rows[2] else {
        panic!("expected tsumo")
    };
    assert!(pai.is_unknown());
    assert!(
        suzume_api::mjai::analyze(&mut std::io::Cursor::new(text), LogLimits::default()).is_ok()
    );
}
