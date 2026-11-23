use super::*;
use crate::game::{Discard, Game};
use crate::tile::{parse_tiles, Tile};

#[test]
fn ai_returns_only_legal_actions() {
    let mut game = Game::new(17).unwrap();
    for _ in 0..200 {
        let Some(seat) = game.next_actor() else {
            break;
        };
        let legal = game.legal_actions(seat);
        let action = GreedyAi
            .choose_action(&game.observation(seat).unwrap(), &legal)
            .unwrap();
        assert!(legal.contains(&action));
        game.apply(seat, action).unwrap();
    }
}

#[test]
fn win_has_priority_over_call_and_pass() {
    let game = Game::new(1).unwrap();
    let mut view = game.observation(1).unwrap();
    view.hand = parse_tiles("222m333p444s55m66s").unwrap();
    view.players[0].hand_len = 13;
    view.dora_indicators = parse_tiles("7z").unwrap();
    view.last_discard = Some((0, Tile::new(4).unwrap()));
    view.players[0].discards.push(Discard {
        tile: Tile::new(4).unwrap(),
        called: false,
        riichi: false,
    });
    assert_eq!(
        GreedyAi.choose_action(&view, &[Action::Pass, Action::Pon, Action::Ron]),
        Some(Action::Ron)
    );
    assert_eq!(GreedyAi.choose_action(&view, &[]), None);
}

#[test]
fn chi_with_a_terminal_is_not_treated_as_open_tanyao() {
    let game = Game::new(1).unwrap();
    let mut view = game.observation(0).unwrap();
    view.hand = parse_tiles("13m234456p34556s").unwrap();
    view.players[0].hand_len = 13;
    view.dora_indicators = parse_tiles("7z").unwrap();
    view.last_discard = Some((3, Tile::new(1).unwrap()));
    view.players[3].discards.push(Discard {
        tile: Tile::new(1).unwrap(),
        called: false,
        riichi: false,
    });
    let chi = Action::Chi([Tile::new(0).unwrap(), Tile::new(2).unwrap()]);
    assert_eq!(
        GreedyAi.choose_action(&view, &[Action::Pass, chi]),
        Some(Action::Pass)
    );
}
