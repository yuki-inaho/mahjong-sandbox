use suzume::{
    ai::{AiPreset, ConfiguredAi},
    game::Action,
};
use suzume_api::{
    actions::{action_id, legal_mask, resolve_action, ACTION_SPACE},
    environment::Environment,
    types::{ActionId, ErrorCode, Seed},
};

#[test]
fn every_legal_action_roundtrips_to_a_legal_canonical_action_through_a_complete_match() {
    let mut environment = Environment::new(&Seed::from(0), 10_000, 1).unwrap();
    let ai = ConfiguredAi::new(AiPreset::Standard.resolve(0)).unwrap();
    let mut action_kinds = std::collections::BTreeSet::new();
    let reserved = (129..135).chain(156..162).chain(183..189);
    let reserved: Vec<usize> = reserved.collect();
    loop {
        let game = environment.game();
        let actor = game.next_actor().unwrap();
        let legal = game.legal_actions(actor);
        let mask = legal_mask(game, actor);
        assert_eq!(mask.len(), ACTION_SPACE);
        assert!(reserved.iter().all(|id| !mask[*id]));
        assert!(legal_mask(game, (actor + 1) % 4).iter().all(|value| !value));
        for action in &legal {
            let id = action_id(game, actor, action).expect("all core legal actions have an ID");
            assert!(mask[usize::from(id.get())]);
            let canonical = resolve_action(game, actor, id).unwrap();
            assert!(legal.contains(&canonical));
            assert_eq!(action_id(game, actor, &canonical), Some(id));
            let kind = match action {
                Action::Discard(_) => "discard",
                Action::Riichi(_) => "riichi",
                Action::Tsumo => "tsumo",
                Action::Ron => "ron",
                Action::Pon => "pon",
                Action::Chi(_) => "chi",
                Action::OpenKan => "open_kan",
                Action::ClosedKan(_) => "closed_kan",
                Action::Pass => "pass",
                Action::NextRound => "next_round",
            };
            action_kinds.insert(kind);
        }
        let distinct_ids: std::collections::BTreeSet<_> = legal
            .iter()
            .map(|action| action_id(game, actor, action).unwrap().get())
            .collect();
        assert_eq!(
            mask.iter().filter(|allowed| **allowed).count(),
            distinct_ids.len()
        );
        let decision = ai
            .decide(&game.observation(actor).unwrap(), &legal)
            .unwrap();
        let id = action_id(game, actor, &decision.action).unwrap();
        let transition = environment.step(id).unwrap();
        assert_eq!(
            transition.rewards.iter().sum::<i32>() + transition.escrow_delta,
            0
        );
        if transition.terminated {
            assert!(!transition.truncated);
            assert!(transition.observation.is_none());
            break;
        }
        assert!(
            !transition.truncated,
            "complete seeded match must finish within its operation limit"
        );
    }
    for expected in ["discard", "riichi", "pass", "next_round"] {
        assert!(action_kinds.contains(expected));
    }
    let before = environment.clone();
    assert_eq!(
        environment
            .step(ActionId::new(72).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::StepLimit
    );
    assert_eq!(environment, before);
}
