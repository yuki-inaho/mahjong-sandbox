use std::process::Command;

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_suzume-ai-eval"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn evaluation_rejects_holdout_seeds_in_development_mode() {
    let output = run(&["--development", "--seed", "1000"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("固定評価用の種"));
}

#[test]
fn evaluation_rejects_overriding_the_frozen_experiment() {
    let output = run(&["--holdout", "--budget", "1", "--output", "unused.csv"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("設定の上書き"));
}

#[test]
fn evaluation_reports_frozen_denominators_and_resolved_configs() {
    let output = run(&["--print-protocol"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"games_per_group\": 32"));
    assert!(text.contains("\"total_games\": 64"));
    assert!(text.contains("\"algorithm\":\"ukeire\""));
    assert!(text.contains("\"algorithm\":\"random\""));
    assert!(text.contains("\"source_fnv1a64\""));
}
