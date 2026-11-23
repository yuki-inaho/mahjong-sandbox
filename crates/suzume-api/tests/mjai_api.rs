use serde_json::{json, Value};
use suzume_api::Engine;

const MJSON: &str = include_str!("../../../examples/eight-actions.mjson");

fn request(engine: &mut Engine, value: Value) -> Value {
    serde_json::from_str(&engine.request(&value.to_string())).unwrap()
}

fn annotated() -> String {
    MJSON
        .lines()
        .enumerate()
        .map(|(offset, event)| {
            format!(
                "{{\"record\":\"annotated_mjai\",\"schema_version\":1,\"index\":{},\"event\":{},\"annotations\":{{\"private\":\"ANNOTATION_PRIVATE_SENTINEL\",\"tiny\":1e-400}}}}\n",
                offset + 1,
                event
            )
        })
        .collect()
}

#[test]
fn standard_and_annotated_requests_match_for_every_viewer_without_mutating_sessions() {
    let mut engine = Engine::new();
    let before = request(&mut engine, json!({"version":1,"op":"new"}));
    let wrapped = annotated();
    for viewer in [Value::Null, json!(0), json!(1), json!(2), json!(3)] {
        let expected = request(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","mjson":MJSON,"viewer":viewer}),
        );
        assert_eq!(expected["status"], "ok", "{expected}");
        for (format, data) in [("mjai", MJSON), ("annotated_mjai", wrapped.as_str())] {
            let actual = request(
                &mut engine,
                json!({"version":1,"op":"analyze_mjai","format":format,"mjson":data,"viewer":viewer}),
            );
            assert_eq!(actual, expected);
            assert!(!actual.to_string().contains("ANNOTATION_PRIVATE_SENTINEL"));
            assert!(actual["data"]["observation"].get("event").is_none());
            assert!(actual["data"]["observation"].get("annotations").is_none());
        }
    }
    assert_eq!(
        request(&mut engine, json!({"version":1,"op":"view"})),
        before
    );
    let environment = request(
        &mut engine,
        json!({"version":1,"op":"environment_reset","seed":"17","max_steps":10}),
    );
    assert_eq!(
        request(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","format":"annotated_mjai","mjson":wrapped})
        )["status"],
        "ok"
    );
    assert_eq!(
        request(&mut engine, json!({"version":1,"op":"environment_view"})),
        environment
    );
}

#[test]
fn unknown_formats_and_wrong_explicit_formats_are_rejected() {
    let mut engine = Engine::new();
    for format in [json!("other"), json!(null), json!(1)] {
        let result = request(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","format":format,"mjson":MJSON}),
        );
        assert_eq!(result["status"], "error");
        assert_eq!(result["code"], "invalid_request");
    }
    for (format, data) in [("annotated_mjai", MJSON), ("mjai", annotated().as_str())] {
        let result = request(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","format":format,"mjson":data}),
        );
        assert_eq!(result["status"], "error");
        assert_eq!(result["code"], "log_error");
    }
}

#[test]
fn annotated_invalid_inputs_preserve_the_active_session_and_report_a_line() {
    let mut engine = Engine::new();
    let before = request(&mut engine, json!({"version":1,"op":"new"}));
    let wrapped = annotated();
    for input in [
        wrapped.replacen("\"schema_version\":1", "\"schema_version\":1.0", 1),
        wrapped.replacen("\"index\":2", "\"index\":3", 1),
        wrapped.replacen("\"annotations\":{", "\"annotations\":{\"x\":1,\"x\":2,", 1),
        wrapped.lines().take(2).collect::<Vec<_>>().join("\n"),
    ] {
        let result = request(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","format":"annotated_mjai","mjson":input}),
        );
        assert_eq!(result["status"], "error");
        assert_eq!(result["code"], "log_error");
        assert!(result["message"].as_str().unwrap().contains("line"));
        assert_eq!(
            request(&mut engine, json!({"version":1,"op":"view"})),
            before
        );
    }
}
