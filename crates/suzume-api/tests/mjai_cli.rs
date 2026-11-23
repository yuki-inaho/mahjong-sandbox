use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzume-mjai-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn synthetic() -> String {
    [json!({"type":"start_game"}),json!({"type":"start_kyoku","bakaze":"E","dora_marker":"1m","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"scores":[25000,25000,25000,25000],"tehais":[vec!["?";13],vec!["?";13],vec!["?";13],vec!["?";13]]}),json!({"type":"tsumo","actor":0,"pai":"5mr"}),json!({"type":"end_game","_suzume":{"reason":"interrupted"}})].into_iter().map(|row| row.to_string()+"\n").collect()
}
#[test]
fn cli_analyzes_plain_and_gzip_by_content() {
    let dir = Directory::new();
    let plain = dir.0.join("plain.data");
    let gzip = dir.0.join("compressed.mjson");
    fs::write(&plain, synthetic()).unwrap();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(synthetic().as_bytes()).unwrap();
    fs::write(&gzip, enc.finish().unwrap()).unwrap();
    for path in [plain, gzip] {
        let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
            .args(["mjai", "analyze"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(result["records"], 4);
        assert_eq!(result["complete"], false);
        assert_eq!(result["red_events"], 1);
    }
}
#[test]
fn cli_converts_legacy_and_refuses_existing_destination() {
    let dir = Directory::new();
    let out = dir.0.join("converted.mjson");
    let input =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/replay-eight-actions.jsonl");
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_suzume-api"))
            .args(["mjai", "convert"])
            .arg(&input)
            .arg("--output")
            .arg(&out)
            .output()
            .unwrap()
    };
    let first = invoke();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let original = fs::read(&out).unwrap();
    assert!(String::from_utf8_lossy(&original).contains("start_kyoku"));
    assert!(!invoke().status.success());
    assert_eq!(fs::read(&out).unwrap(), original);
}
#[test]
fn corrupt_tail_never_publishes_observations() {
    let dir = Directory::new();
    let input = dir.0.join("corrupt.mjson");
    let out = dir.0.join("observations.jsonl");
    fs::write(&input, synthetic() + "{broken}\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(["mjai", "observations"])
        .arg(&input)
        .args(["--seat", "0", "--output"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!out.exists());
    assert!(!fs::read_dir(&dir.0).unwrap().any(|entry| entry
        .unwrap()
        .path()
        .extension()
        .is_some_and(|v| v == "pending")));
}
#[test]
fn observations_do_not_expose_other_hands() {
    let dir = Directory::new();
    let input = dir.0.join("full.mjson");
    let out = dir.0.join("observations.jsonl");
    fs::write(&input, synthetic()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(["mjai", "observations"])
        .arg(&input)
        .args(["--seat", "1", "--output"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = fs::read_to_string(out).unwrap();
    assert!(!text.contains("5mr"));
    assert!(!text.contains("\"tehais\""));
}
#[test]
fn cli_rejects_truncated_gzip() {
    let dir = Directory::new();
    let input = dir.0.join("broken.mjson");
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(synthetic().as_bytes()).unwrap();
    let mut bytes = enc.finish().unwrap();
    bytes.truncate(bytes.len() - 4);
    fs::write(&input, bytes).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(["mjai", "analyze"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[test]
fn gzip_cannot_bypass_decompressed_line_limit() {
    let dir = Directory::new();
    let input = dir.0.join("oversized.mjson");
    let body = format!(
        "{{\"type\":\"start_game\",\"memo\":\"{}\"}}\n",
        "a".repeat(1_048_577)
    );
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(body.as_bytes()).unwrap();
    fs::write(&input, enc.finish().unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(["mjai", "analyze"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("line byte limit"));
}
