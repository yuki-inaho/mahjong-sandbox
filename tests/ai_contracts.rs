use suzume::ai::{AiAlgorithm, AiConfig, AiError, AiPreset, ConfiguredAi, GreedyAi, Policy};
use suzume::game::{Action, Discard, Game};
use suzume::scoring::{Meld, MeldKind};
use suzume::tile::{parse_tiles, Tile};

fn ukeire(budget: u32, depth: u8) -> ConfiguredAi {
    ConfiguredAi::new(AiConfig {
        algorithm: AiAlgorithm::Ukeire,
        seed: 23,
        max_evaluations: budget,
        lookahead_depth: depth,
        risk_weight: 4,
    })
    .unwrap()
}

#[test]
fn ai_ukeire_avoids_publicly_exhausted_wait() {
    let game = Game::new(0).unwrap();
    let mut view = game.observation(0).unwrap();
    view.hand = parse_tiles("123456m123p678s12z").unwrap();
    view.dora_indicators = parse_tiles("7z").unwrap();
    view.players[1].discards = vec![
        Discard {
            tile: Tile::new(28).unwrap(),
            called: false,
            riichi: false
        };
        3
    ];
    let legal = [Action::Riichi(12), Action::Riichi(13)];
    let decision = ukeire(2048, 1).decide(&view, &legal).unwrap();
    assert_eq!(decision.action, Action::Riichi(13));
    assert_eq!(decision.candidates[0].ukeire, Some(0));
    assert_eq!(decision.candidates[1].ukeire, Some(3));
}

#[test]
fn ai_rejects_invalid_observations_and_action_indices() {
    let game = Game::new(0).unwrap();
    let view = game.observation(0).unwrap();
    let ai = ukeire(1024, 1);
    assert!(matches!(
        ai.decide(&view, &[Action::Discard(99)]),
        Err(AiError::InvalidAction(_))
    ));
    assert_eq!(GreedyAi.choose_action(&view, &[Action::Discard(99)]), None);
    let mut invalid = view.clone();
    invalid.seat = usize::MAX;
    assert!(matches!(
        ai.decide(&invalid, &[Action::Discard(0)]),
        Err(AiError::InvalidObservation(_))
    ));
    invalid = view.clone();
    invalid.players[0].melds.push(Meld {
        kind: MeldKind::Pon,
        tiles: vec![],
        from: Some(1),
    });
    assert!(matches!(
        ai.decide(&invalid, &[Action::Discard(0)]),
        Err(AiError::InvalidObservation(_))
    ));
    assert!(matches!(
        ai.decide(&view, &[]),
        Err(AiError::NoLegalActions)
    ));
}

#[test]
fn ai_is_deterministic_for_same_inputs_and_seed() {
    let game = Game::new(3).unwrap();
    let view = game.observation(0).unwrap();
    let legal = game.legal_actions(0);
    for preset in [AiPreset::Beginner, AiPreset::Standard, AiPreset::Strong] {
        let ai = ConfiguredAi::new(preset.resolve(42)).unwrap();
        assert_eq!(
            ai.decide(&view, &legal).unwrap(),
            ai.decide(&view, &legal).unwrap()
        );
    }
    let choices: Vec<Action> = (0..16)
        .map(|seed| {
            ConfiguredAi::new(AiPreset::Beginner.resolve(seed))
                .unwrap()
                .decide(&view, &legal)
                .unwrap()
                .action
        })
        .collect();
    assert!(choices.iter().any(|action| *action != choices[0]));
}

#[test]
fn ai_never_exceeds_declared_evaluation_budget() {
    let game = Game::new(0).unwrap();
    let view = game.observation(0).unwrap();
    let legal = game.legal_actions(0);
    for budget in [1, 7, 64, 1024, 8192] {
        let decision = ukeire(budget, 2).decide(&view, &legal).unwrap();
        assert!(legal.contains(&decision.action));
        assert!(decision.evaluations <= budget);
        assert_eq!(
            decision.candidates_evaluated as usize,
            decision
                .candidates
                .iter()
                .filter(|c| c.shanten.is_some())
                .count()
        );
        if budget == 1 {
            assert!(decision.budget_exhausted);
            assert!(decision.candidates.iter().any(|c| c.ukeire.is_none()));
        }
    }
}

#[test]
fn ai_public_tile_counts_do_not_double_count_called_discard() {
    let game = Game::new(0).unwrap();
    let mut view = game.observation(0).unwrap();
    view.hand = parse_tiles("123m123p455s11z").unwrap();
    let meld = Meld {
        kind: MeldKind::Pon,
        tiles: parse_tiles("222s").unwrap(),
        from: Some(3),
    };
    view.melds = vec![meld.clone()];
    view.players[0].melds = vec![meld];
    view.players[0].hand_len = view.hand.len();
    view.players[3].discards = vec![Discard {
        tile: Tile::new(19).unwrap(),
        called: true,
        riichi: false,
    }];
    view.dora_indicators = parse_tiles("2s").unwrap();
    assert!(ukeire(1024, 1).decide(&view, &[Action::Discard(0)]).is_ok());
}

#[test]
fn ai_depth_two_reports_measured_lookahead() {
    let game = Game::new(0).unwrap();
    let view = game.observation(0).unwrap();
    let legal = game.legal_actions(0);
    let shallow = ukeire(8192, 1).decide(&view, &legal).unwrap();
    let deeper = ukeire(8192, 2).decide(&view, &legal).unwrap();
    assert!(deeper.evaluations > shallow.evaluations);
    assert!(deeper
        .candidates
        .iter()
        .any(|candidate| candidate.lookahead_value.is_some()));
}

#[test]
fn ai_does_not_treat_a_fifth_owned_tile_as_tenpai() {
    let game = Game::new(0).unwrap();
    let mut view = game.observation(0).unwrap();
    view.hand = parse_tiles("1m234p567789s2z").unwrap();
    let meld = Meld {
        kind: MeldKind::Pon,
        tiles: parse_tiles("111m").unwrap(),
        from: Some(1),
    };
    view.melds = vec![meld.clone()];
    view.players[0].melds = vec![meld];
    view.players[0].hand_len = view.hand.len();
    view.players[1].discards = vec![Discard {
        tile: Tile::new(0).unwrap(),
        called: true,
        riichi: false,
    }];
    view.dora_indicators = parse_tiles("7z").unwrap();
    let decision = ukeire(2048, 1)
        .decide(&view, &[Action::Discard(10), Action::Discard(0)])
        .unwrap();
    assert_eq!(decision.candidates[0].shanten, Some(1));
    assert_eq!(decision.candidates[1].shanten, Some(0));
    assert_eq!(decision.action, Action::Discard(0));
}

#[test]
fn ai_config_rejects_conflicting_algorithm_parameters() {
    assert!("unknown".parse::<AiAlgorithm>().is_err());
    assert!("expert".parse::<AiPreset>().is_err());
    let mut config = AiPreset::Beginner.resolve(0);
    config.max_evaluations = 1;
    assert!(config.validate().is_err());
    config = AiPreset::Strong.resolve(0);
    config.lookahead_depth = 3;
    assert!(config.validate().is_err());
    config.lookahead_depth = 2;
    config.max_evaluations = 100_001;
    assert!(config.validate().is_err());
}

#[test]
fn ai_all_algorithms_stay_legal_on_real_transitions() {
    for (seed, preset) in [
        (31, AiPreset::Beginner),
        (37, AiPreset::Standard),
        (43, AiPreset::Strong),
    ] {
        let mut game = Game::new(seed).unwrap();
        let ai = ConfiguredAi::new(preset.resolve(5)).unwrap();
        for _ in 0..100 {
            let Some(seat) = game.next_actor() else {
                break;
            };
            let legal = game.legal_actions(seat);
            let decision = ai.decide(&game.observation(seat).unwrap(), &legal).unwrap();
            assert!(legal.contains(&decision.action));
            game.apply(seat, decision.action).unwrap();
        }
    }
}
