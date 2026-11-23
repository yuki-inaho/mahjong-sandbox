//! 外部連携の拡張に先立ち、対局の公開操作から確認した不具合を固定します。

use super::*;
use crate::tile::parse_tiles;

fn one(text: &str) -> Tile {
    let tiles = parse_tiles(text).unwrap();
    assert_eq!(tiles.len(), 1);
    tiles[0]
}

/// 各種4枚の単一の牌プールから配分し、検査に必要な3人の牌を確保します。
fn called_discard_fixture() -> Game {
    let mut game = Game::new(0).unwrap();
    let mut pool: Vec<Tile> = (0..34).flat_map(|id| [tile(id); 4]).collect();
    for (seat, text) in ["", "67888m567888p44s", "5m4s", "8p"]
        .into_iter()
        .enumerate()
    {
        game.players[seat] = Player::new(format!("検査席{seat}"));
        for tile in parse_tiles(text).unwrap() {
            let index = pool
                .iter()
                .position(|&candidate| candidate == tile)
                .unwrap();
            game.players[seat].hand.push(pool.remove(index));
        }
    }
    game.dead_wall = pool.split_off(pool.len() - DEAD_WALL);
    for (seat, player) in game.players.iter_mut().enumerate() {
        let required = if seat == 2 { 14 } else { 13 };
        while player.hand.len() < required {
            player.hand.push(pool.pop().unwrap());
        }
        player.hand.sort_unstable();
    }
    game.wall = pool;
    game.phase = Phase::Discard { player: 2 };
    game.last_draw = Some((2, one("4s"), false));
    game.validate().unwrap();
    game
}

fn discard(game: &mut Game, seat: usize, text: &str) {
    let index = game.players[seat]
        .hand
        .iter()
        .position(|&tile| tile == one(text))
        .unwrap();
    game.apply(seat, Action::Discard(index)).unwrap();
}

fn respond(game: &mut Game, pon: Option<usize>) {
    while matches!(game.phase, Phase::Reaction { .. }) {
        let seat = game.next_actor().unwrap();
        let action = if pon == Some(seat) {
            Action::Pon
        } else {
            Action::Pass
        };
        game.apply(seat, action).unwrap();
    }
}

#[test]
fn temporary_furiten_expires_after_called_discard() {
    let mut game = called_discard_fixture();
    discard(&mut game, 2, "4s");
    assert!(game.legal_actions(1).contains(&Action::Ron));
    respond(&mut game, None);
    assert!(game.players[1].temporary_furiten);

    discard(&mut game, 3, "8p");
    respond(&mut game, Some(1));
    assert!(game.players[1].temporary_furiten);
    discard(&mut game, 1, "5p");
    respond(&mut game, None);

    discard(&mut game, 2, "5m");
    game.validate().unwrap();
    assert!(game.score(1, false, one("5m")).is_some());
    assert!(
        game.legal_actions(1).contains(&Action::Ron),
        "非リーチの同巡振聴は、鳴きに続く自分の打牌でも解除される"
    );
    assert!(!game.players[1].temporary_furiten);
}
