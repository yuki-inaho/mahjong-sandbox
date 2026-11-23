use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzume-annotated-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn assert_unpublished(&self, destination: &Path) {
        assert!(!destination.exists());
        assert!(!fs::read_dir(&self.0).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "pending")));
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(arguments)
        .output()
        .unwrap()
}

fn success(result: Output) -> Value {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn minimal() -> &'static str {
    "{ \"extra\":{\"big\":9007199254740993e0,\"tiny\":1e-400,\"null\":null}, \"type\":\"start_game\" }\n{\"type\":\"end_game\"}\n"
}

fn annotated(text: &str, note: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(index, event)| {
            format!(
                "{{\"record\":\"annotated_mjai\",\"schema_version\":1,\"index\":{},\"event\":{},\"annotations\":{note}}}\n",
                index + 1,
                event
            )
        })
        .collect()
}

fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn wrap_and_unwrap_keep_original_json_and_refuse_overwrite() {
    let directory = Directory::new();
    let input = directory.0.join("input.mjson");
    let wrapped = directory.0.join("wrapped.annotated.jsonl");
    let output = directory.0.join("output.mjson");
    fs::write(&input, minimal()).unwrap();
    let args = [
        "mjai",
        "wrap",
        input.to_str().unwrap(),
        "--output",
        wrapped.to_str().unwrap(),
    ];
    let report = success(run(&args));
    assert_eq!(report["summary"]["records"], 2);
    assert_eq!(report["dropped_annotation_records"], 0);
    let original = fs::read(&wrapped).unwrap();
    assert!(!run(&args).status.success());
    assert_eq!(fs::read(&wrapped).unwrap(), original);
    let report = success(run(&[
        "mjai",
        "unwrap",
        wrapped.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]));
    assert_eq!(report["summary"]["records"], 2);
    assert_eq!(fs::read_to_string(output).unwrap(), minimal());
}

#[test]
fn annotations_require_explicit_discard_and_report_affected_rows() {
    let directory = Directory::new();
    let input = directory.0.join("notes.annotated.jsonl");
    let output = directory.0.join("output.mjson");
    fs::write(
        &input,
        annotated(minimal(), "{\"explanation\":\"解析理由\"}"),
    )
    .unwrap();
    let result = run(&[
        "mjai",
        "unwrap",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("annotation"));
    directory.assert_unpublished(&output);
    let report = success(run(&[
        "mjai",
        "unwrap",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--drop-annotations",
    ]));
    assert_eq!(report["dropped_annotation_records"], 2);
    assert_eq!(fs::read_to_string(output).unwrap(), minimal());
}

#[test]
fn annotated_analysis_and_observations_match_standard_without_annotations() {
    let directory = Directory::new();
    let standard = directory.0.join("input.mjson");
    let wrapped = directory.0.join("input.annotated.jsonl");
    let text = [
        json!({"type":"start_game"}),
        json!({"type":"start_kyoku","bakaze":"E","dora_marker":"1m","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"scores":[25000,25000,25000,25000],"tehais":[vec!["?";13],vec!["?";13],vec!["?";13],vec!["?";13]]}),
        json!({"type":"tsumo","actor":0,"pai":"5mr"}),
        json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
    ].into_iter().map(|event| event.to_string()+"\n").collect::<String>();
    fs::write(&standard, &text).unwrap();
    fs::write(
        &wrapped,
        annotated(
            &text,
            "{\"private_note\":\"hidden-analysis\",\"tag\":\"5mr\"}",
        ),
    )
    .unwrap();
    let expected = success(run(&["mjai", "analyze", standard.to_str().unwrap()]));
    let actual = success(run(&[
        "mjai",
        "analyze",
        wrapped.to_str().unwrap(),
        "--format",
        "annotated_mjai",
    ]));
    assert_eq!(expected, actual);
    assert_eq!(actual["red_events"], 1);
    for seat in 0..4 {
        let seat = seat.to_string();
        let plain_out = directory.0.join(format!("plain-{seat}.jsonl"));
        let wrapped_out = directory.0.join(format!("wrapped-{seat}.jsonl"));
        let plain = run(&[
            "mjai",
            "observations",
            standard.to_str().unwrap(),
            "--seat",
            &seat,
            "--output",
            plain_out.to_str().unwrap(),
        ]);
        assert!(plain.status.success());
        let result = run(&[
            "mjai",
            "observations",
            wrapped.to_str().unwrap(),
            "--seat",
            &seat,
            "--output",
            wrapped_out.to_str().unwrap(),
            "--format",
            "annotated_mjai",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual = fs::read_to_string(wrapped_out).unwrap();
        assert_eq!(actual, fs::read_to_string(plain_out).unwrap());
        assert!(!actual.contains("private_note"));
        assert!(!actual.contains("hidden-analysis"));
        assert!(!actual.contains("tehais"));
        if seat != "0" {
            assert!(!actual.contains("5mr"));
        }
    }
}

#[test]
fn gzip_is_detected_for_both_conversion_directions() {
    let directory = Directory::new();
    for (operation, text) in [
        ("wrap", minimal().to_owned()),
        ("unwrap", annotated(minimal(), "{}")),
    ] {
        let input = directory.0.join(format!("{operation}.data"));
        let output = directory.0.join(format!("{operation}.output"));
        fs::write(&input, gzip(&text)).unwrap();
        let report = success(run(&[
            "mjai",
            operation,
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ]));
        assert_eq!(report["summary"]["records"], 2);
    }
}

#[test]
fn late_corruption_and_truncated_gzip_never_publish_converted_files() {
    let directory = Directory::new();
    for (operation, text) in [
        ("wrap", minimal().to_owned()),
        ("unwrap", annotated(minimal(), "{}")),
    ] {
        for corruption in ["missing_end", "extra_tail", "broken_gzip"] {
            let input = directory.0.join(format!("{operation}-{corruption}.input"));
            let output = directory.0.join(format!("{operation}-{corruption}.output"));
            let bytes = match corruption {
                "missing_end" => text.lines().next().unwrap().as_bytes().to_vec(),
                "extra_tail" => (text.clone() + "{broken}\n").into_bytes(),
                _ => {
                    let mut bytes = gzip(&text);
                    bytes.truncate(bytes.len() - 4);
                    bytes
                }
            };
            fs::write(&input, bytes).unwrap();
            let result = run(&[
                "mjai",
                operation,
                input.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ]);
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("line"));
            assert!(result.stdout.is_empty());
            directory.assert_unpublished(&output);
        }
    }
}

#[test]
fn wrapping_refuses_output_that_exceeds_line_limit() {
    let directory = Directory::new();
    let input = directory.0.join("near-limit.mjson");
    let output = directory.0.join("over-limit.annotated.jsonl");
    let text = format!(
        "{{\"type\":\"start_game\",\"memo\":\"{}\"}}\n{{\"type\":\"end_game\"}}\n",
        "a".repeat(1_048_576 - 80)
    );
    assert!(text.lines().next().unwrap().len() < 1_048_576);
    fs::write(&input, text).unwrap();
    let result = run(&[
        "mjai",
        "wrap",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("line byte limit"));
    directory.assert_unpublished(&output);
}

#[cfg(target_os = "linux")]
#[test]
fn report_failure_explicitly_identifies_the_already_published_file() {
    let directory = Directory::new();
    let input = directory.0.join("input.mjson");
    let output = directory.0.join("published.annotated.jsonl");
    fs::write(&input, minimal()).unwrap();
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_suzume-api"))
        .args(["mjai", "wrap"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .stdout(full)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(output.is_file());
    let message = String::from_utf8_lossy(&result.stderr);
    assert!(message.contains("公開済み"), "{message}");
    assert!(message.contains(output.to_str().unwrap()), "{message}");
    let saved = success(run(&[
        "mjai",
        "analyze",
        output.to_str().unwrap(),
        "--format",
        "annotated_mjai",
    ]));
    assert_eq!(saved["records"], 2);
}
