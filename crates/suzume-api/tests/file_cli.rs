use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzume-file-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/replay-eight-actions.jsonl")
}

#[test]
fn replay_and_dataset_use_the_shipped_recording() {
    let directory = TestDirectory::new();
    let output = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .arg("replay")
        .arg(fixture())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["action_count"], 8);
    assert_eq!(summary["reason"], "truncated");
    let destination = directory.0.join("dataset.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .arg("dataset")
        .arg(fixture())
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Vec<Value> = fs::read_to_string(&destination)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[7]["truncated"], true);
    let previous = fs::read(&destination).unwrap();
    let duplicate = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .arg("dataset")
        .arg(fixture())
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert_eq!(previous, fs::read(&destination).unwrap());
}

#[test]
fn malformed_late_input_does_not_publish_a_partial_dataset() {
    let directory = TestDirectory::new();
    let original = fs::read_to_string(fixture()).unwrap();
    let input = directory.0.join("truncated.jsonl");
    let lines: Vec<&str> = original.lines().collect();
    fs::write(&input, lines[..lines.len() - 1].join("\n")).unwrap();
    let destination = directory.0.join("must-not-exist.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .arg("dataset")
        .arg(&input)
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}
