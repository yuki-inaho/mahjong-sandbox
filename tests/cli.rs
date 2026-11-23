//! 外部プロセスとして起動し、表示モードと入力境界を検査します。

use std::io::Write;
use std::process::{Command, Stdio};

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
