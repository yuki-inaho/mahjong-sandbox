use serde_json::{json, Value};
use std::io::{self, Cursor, Write};
use suzume_api::{transport::serve, MAX_REQUEST_BYTES};

#[test]
fn oversized_invalid_utf8_and_malformed_lines_do_not_desynchronize_transport() {
    let mut bytes = vec![b' '; MAX_REQUEST_BYTES + 1];
    bytes.extend_from_slice(
        b"\n\xff\n{\n{\"version\":1,\"op\":\"new\"}\n{\"version\":1,\"op\":\"view\"}",
    );
    let mut output = Vec::new();
    serve(&mut Cursor::new(bytes), &mut output).unwrap();
    let lines: Vec<Value> = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 5);
    for response in &lines[..3] {
        assert_eq!(response["code"], "invalid_request");
    }
    assert_eq!(lines[3]["status"], "ok");
    assert_eq!(lines[3], lines[4]);
}

#[test]
fn too_large_or_invalid_configuration_preserves_the_active_game() {
    let mut engine = suzume_api::Engine::new();
    let before: Value =
        serde_json::from_str(&engine.request(r#"{"version":1,"op":"new"}"#)).unwrap();
    let too_large = engine.request(&" ".repeat(MAX_REQUEST_BYTES + 1));
    assert_eq!(
        serde_json::from_str::<Value>(&too_large).unwrap()["code"],
        "invalid_request"
    );
    let invalid = engine
        .request(&json!({"version":1,"op":"load_config","toml":"schema_version = 2"}).to_string());
    assert_eq!(
        serde_json::from_str::<Value>(&invalid).unwrap()["code"],
        "invalid_config"
    );
    assert_eq!(
        before,
        serde_json::from_str::<Value>(&engine.request(r#"{"version":1,"op":"view"}"#)).unwrap()
    );
}

struct FailingWriter;
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("intentional writer failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn output_failure_is_propagated() {
    let error = serve(
        &mut Cursor::new(b"{\"version\":1,\"op\":\"new\"}\n"),
        &mut FailingWriter,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
}
