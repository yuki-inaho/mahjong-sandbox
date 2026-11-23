use serde_json::{json, Value};
use suzume_api::Engine;

fn request(engine: &mut Engine, value: Value) -> Value {
    serde_json::from_str(&engine.request(&value.to_string())).unwrap()
}

#[test]
fn new_game_hides_other_hands_and_seed() {
    let mut engine = Engine::new();
    let result = request(&mut engine, json!({"version":1,"op":"new"}));
    assert_eq!(result["status"], "ok");
    let state = &result["data"]["state"];
    assert_eq!(state["revision"], 0);
    assert_eq!(state["own_hand"].as_array().unwrap().len(), 14);
    assert_eq!(state["players"].as_array().unwrap().len(), 4);
    for player in state["players"].as_array().unwrap() {
        assert!(player.get("hand").is_none());
    }
    for private in ["seed", "wall", "dead_wall", "history", "responses"] {
        assert!(state.get(private).is_none(), "{private}");
    }
}

#[test]
fn stale_and_illegal_requests_leave_every_observable_field_unchanged() {
    let mut engine = Engine::new();
    let first = request(&mut engine, json!({"version":1,"op":"new"}));
    assert_eq!(first["status"], "ok");
    let state = &first["data"]["state"];
    let id = state["session_id"].clone();
    let rejected = request(
        &mut engine,
        json!({"version":1,"op":"apply","session_id":id,
        "expected_revision":999,"action":{"kind":"discard","index":0}}),
    );
    assert_eq!(rejected["code"], "stale_revision");
    let illegal = request(
        &mut engine,
        json!({"version":1,"op":"apply","session_id":id,
        "expected_revision":0,"action":{"kind":"ron"}}),
    );
    assert_eq!(illegal["code"], "illegal_action");
    let after = request(&mut engine, json!({"version":1,"op":"view"}));
    assert_eq!(first, after);
}

#[test]
fn malformed_unknown_and_unsupported_requests_are_explicit_errors() {
    let mut engine = Engine::new();
    for input in [
        "{",
        "null",
        r#"{"version":2,"op":"new"}"#,
        r#"{"version":1,"op":"new","typo":true}"#,
        r#"{"version":1,"version":1,"op":"new"}"#,
    ] {
        let result: Value = serde_json::from_str(&engine.request(input)).unwrap();
        assert_eq!(result["status"], "error");
        assert_ne!(result["code"], "not_ready");
    }
}

#[test]
fn old_session_action_cannot_affect_a_restarted_game() {
    let mut engine = Engine::new();
    let first = request(&mut engine, json!({"version":1,"op":"new"}));
    assert_eq!(first["status"], "ok");
    let old = first["data"]["state"]["session_id"].clone();
    let second = request(&mut engine, json!({"version":1,"op":"new"}));
    let error = request(
        &mut engine,
        json!({"version":1,"op":"apply","session_id":old,
        "expected_revision":0,"action":{"kind":"discard","index":0}}),
    );
    assert_eq!(error["code"], "stale_session");
    assert_eq!(
        second,
        request(&mut engine, json!({"version":1,"op":"view"}))
    );
}
