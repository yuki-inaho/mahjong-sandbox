use suzume::ai::{GreedyAi, Policy};
use suzume::game::{Action, Game, Phase};

#[test]
fn illegal_action_is_atomic() {
    let mut game = Game::new(5).unwrap();
    let before = game.clone();
    assert!(game.apply(4, Action::Discard(0)).is_err());
    assert_eq!(game, before);
    assert!(game.apply(0, Action::Discard(99)).is_err());
    assert_eq!(game, before);
    assert!(game.apply(1, Action::Discard(0)).is_err());
    assert_eq!(game, before);
    assert!(game.apply(0, Action::Ron).is_err());
    assert_eq!(game, before);
}

#[test]
fn seeded_games_preserve_invariants() {
    for seed in 0..4 {
        let mut game = Game::new(seed).unwrap();
        let mut finished = false;
        for _ in 0..10_000 {
            game.validate().unwrap();
            let Some(seat) = game.next_actor() else {
                finished = true;
                break;
            };
            let legal = game.legal_actions(seat);
            assert!(!legal.is_empty(), "seed={seed}, state={:?}", game.phase());
            let action = GreedyAi
                .choose_action(&game.observation(seat).unwrap(), &legal)
                .unwrap();
            game.apply(seat, action).unwrap();
        }
        assert!(
            finished,
            "seed {seed} did not finish within the explicit test limit"
        );
        assert_eq!(
            game.scores().iter().sum::<i32>() + game.riichi_pot(),
            100_000
        );
        assert!(matches!(game.phase(), Phase::MatchEnd));
    }
}

#[test]
fn a_seed_and_action_history_replay_identically() {
    let mut first = Game::new(42).unwrap();
    for _ in 0..80 {
        let seat = first.next_actor().unwrap();
        let legal = first.legal_actions(seat);
        let action = GreedyAi
            .choose_action(&first.observation(seat).unwrap(), &legal)
            .unwrap();
        first.apply(seat, action).unwrap();
    }
    let mut replay = Game::new(42).unwrap();
    for entry in first.history() {
        replay.apply(entry.player, entry.action.clone()).unwrap();
    }
    assert_eq!(replay, first);
}

#[test]
fn opponent_observation_exposes_only_public_information() {
    let game = Game::new(0).unwrap();
    let view = game.observation(1).unwrap();
    assert_eq!(view.hand, game.players()[1].hand);
    assert_eq!(view.players[0].hand_len, 14);
    assert_eq!(view.players[1].hand_len, 13);
    assert!(game.observation(4).is_err());
}
