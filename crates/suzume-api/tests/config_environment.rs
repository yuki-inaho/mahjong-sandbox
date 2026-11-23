use suzume_api::{
    actions::{action_id, legal_mask, resolve_action},
    config::{AiConfigSpec, AppConfig, Controller, PublicViewer, Viewer},
    environment::Environment,
    types::{ActionId, Seed},
};

#[test]
fn configuration_toml_roundtrip_including_public_viewer() {
    let original = AppConfig::default();
    assert_eq!(
        AppConfig::from_toml(&toml::to_string(&original).unwrap()).unwrap(),
        original
    );
    let mut public = original;
    public.game.viewer = Viewer::Public(PublicViewer::Public);
    public.players = std::array::from_fn(|_| Controller::Ai {
        config: AiConfigSpec::default(),
    });
    assert_eq!(
        AppConfig::from_toml(&toml::to_string(&public).unwrap()).unwrap(),
        public
    );
    assert!(toml::to_string(&public).unwrap().contains("public"));
}

#[test]
fn configuration_rejects_unknown_duplicate_conflicting_and_out_of_range_values() {
    for text in [
        "schema_version=2",
        "schema_version=1\ntypo=true",
        "schema_version=1\nschema_version=1",
        "schema_version=1\n[game]\nseed=9007199254740993",
        "schema_version=1\n[game]\nseed=\"18446744073709551616\"",
        "schema_version=1\n[game]\nseed=\"00\"",
        "schema_version=1\n[game]\nmax_steps=0",
    ] {
        assert!(AppConfig::from_toml(text).is_err(), "{text}");
    }
    let mixed: AiConfigSpec =
        serde_json::from_str(r#"{"preset":"strong","algorithm":"random"}"#).unwrap();
    assert!(mixed.resolve().is_err());
    let invalid: AiConfigSpec =
        serde_json::from_str(r#"{"algorithm":"random","max_evaluations":5}"#).unwrap();
    assert!(invalid.resolve().is_err());
    for algorithm in ["random", "greedy", "ukeire"] {
        let config: AiConfigSpec =
            serde_json::from_value(serde_json::json!({"algorithm":algorithm})).unwrap();
        config.resolve().unwrap();
    }
}

#[test]
fn seeds_preserve_the_entire_unsigned_64_bit_domain() {
    for seed in [0, 1, 9007199254740993, u64::MAX] {
        let original = Seed::from(seed);
        let wire = serde_json::to_string(&original).unwrap();
        assert!(wire.starts_with('"'));
        let decoded: Seed = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded.value(), seed);
    }
}

#[test]
fn environment_reset_branch_masks_rewards_and_truncation_are_explicit() {
    let mut a = Environment::new(&Seed::from(42), 3, 1).unwrap();
    let b = a.fork();
    assert_eq!(a, b);
    let before = a.state();
    let obs = before.observation.unwrap();
    assert_eq!(obs.legal_mask.len(), 189);
    assert!(obs.visible_counts.iter().all(|x| *x <= 4));
    for _ in 0..3 {
        let observation = a.state().observation.unwrap();
        let actor = usize::from(observation.actor.get());
        let valid = legal_mask(a.game(), actor);
        for (id, present) in valid.iter().enumerate() {
            let id = ActionId::new(id as u16).unwrap();
            assert_eq!(resolve_action(a.game(), actor, id).is_some(), *present);
            if let Some(action) = resolve_action(a.game(), actor, id) {
                assert_eq!(action_id(a.game(), actor, &action), Some(id));
            }
        }
        let id = valid.iter().position(|x| *x).unwrap();
        let points = a.game().scores();
        let transition = a.step(ActionId::new(id as u16).unwrap()).unwrap();
        assert_eq!(
            transition.rewards,
            std::array::from_fn(|i| a.game().scores()[i] - points[i])
        );
        assert_eq!(
            transition.rewards.iter().sum::<i32>() + transition.escrow_delta,
            0
        );
    }
    assert!(!a.terminated());
    assert!(a.truncated());
    assert!(a.state().observation.is_none());
    let frozen = a.clone();
    assert!(a.step(ActionId::new(0).unwrap()).is_err());
    assert_eq!(a, frozen);
    assert_eq!(b.revision(), 0);
    assert_eq!(Environment::new(&Seed::from(42), 3, 1).unwrap(), b);
}

#[test]
fn invalid_and_reserved_environment_actions_are_atomic() {
    let mut env = Environment::new(&Seed::from(0), 100, 1).unwrap();
    for id in [68, 69, 73, 188] {
        let before = env.clone();
        assert!(env.step(ActionId::new(id).unwrap()).is_err());
        assert_eq!(env, before);
    }
    assert!(ActionId::new(189).is_none());
    assert!(Environment::new(&Seed::from(0), 0, 1).is_err());
}
