//! 物理牌を136枚に保った局面から、公開操作を通して規則を検査します。

use super::*;
use crate::tile::parse_tiles;

fn one(text: &str) -> Tile {
    let tiles = parse_tiles(text).unwrap();
    assert_eq!(tiles.len(), 1);
    tiles[0]
}

fn take(pool: &mut Vec<Tile>, tile: Tile) -> Tile {
    let index = pool
        .iter()
        .position(|&candidate| candidate == tile)
        .expect("局面の牌は各種類4枚以内でなければならない");
    pool.remove(index)
}

/// 指定した手牌を優先し、残りの手牌・王牌・山を同じ136枚から補います。
/// 牌を複製した不正局面を作らないため、全て同じ牌プールから取り出します。
fn fixture(actor: usize, starts: [&str; 4], drawn: &str) -> Game {
    let mut game = Game::new(0).unwrap();
    let mut pool: Vec<Tile> = (0..34).flat_map(|id| [tile(id); 4]).collect();
    for (seat, text) in starts.into_iter().enumerate() {
        game.players[seat] = Player::new(format!("検査席{seat}"));
        for tile in parse_tiles(text).unwrap() {
            game.players[seat].hand.push(take(&mut pool, tile));
        }
    }
    // 第一表示牌は中→白です。基本的な順子手の点数例に偶発ドラを混ぜません。
    let indicators = [take(&mut pool, one("7z")), take(&mut pool, one("7z"))];
    game.dead_wall = (0..14)
        .map(|index| match index {
            4 => indicators[0],
            5 => indicators[1],
            _ => pool.pop().unwrap(),
        })
        .collect();
    for (seat, player) in game.players.iter_mut().enumerate() {
        let expected = if seat == actor { 14 } else { 13 };
        assert!(player.hand.len() <= expected);
        while player.hand.len() < expected {
            player.hand.push(pool.pop().unwrap());
        }
        player.hand.sort_unstable();
    }
    game.wall = pool;
    game.phase = Phase::Discard { player: actor };
    game.last_draw = Some((actor, one(drawn), false));
    assert!(game.players[actor].hand.contains(&one(drawn)));
    game.validate().unwrap();
    game
}

fn discard(game: &mut Game, player: usize, text: &str) {
    let index = game.players[player]
        .hand
        .iter()
        .position(|&tile| tile == one(text))
        .unwrap();
    game.apply(player, Action::Discard(index)).unwrap();
}

fn reactions(game: &mut Game, chosen: &[(usize, Action)]) {
    while matches!(game.phase, Phase::Reaction { .. }) {
        let actor = game.next_actor().unwrap();
        let action = chosen
            .iter()
            .find(|(seat, _)| *seat == actor)
            .map(|(_, action)| action.clone())
            .unwrap_or(Action::Pass);
        game.apply(actor, action).unwrap();
    }
}

fn dead_draw(game: &mut Game, text: &str) {
    let wanted = one(text);
    let index = game.kan_count;
    if let Some(position) = game.wall.iter().position(|&tile| tile == wanted) {
        std::mem::swap(&mut game.wall[position], &mut game.dead_wall[index]);
    } else if let Some(position) = game.dead_wall.iter().position(|&tile| tile == wanted) {
        game.dead_wall.swap(index, position);
    } else {
        let actor = game.next_actor().unwrap();
        let (seat, position) = (0..4)
            .filter(|&seat| seat != actor)
            .find_map(|seat| {
                game.players[seat]
                    .hand
                    .iter()
                    .position(|&tile| tile == wanted)
                    .map(|position| (seat, position))
            })
            .unwrap();
        std::mem::swap(
            &mut game.players[seat].hand[position],
            &mut game.dead_wall[index],
        );
        game.players[seat].hand.sort_unstable();
    }
    game.validate().unwrap();
}

fn shorten_wall(game: &mut Game, remaining: usize, protect: &[usize]) {
    let count = game.wall.len() - remaining;
    assert_eq!(count % 4, 0, "全員が通常の摸打を同じ回数だけ経た局面を作る");
    let turns = count / 4;
    let seats = (0..4)
        .filter(|seat| protect.contains(seat))
        .chain((0..4).filter(|seat| !protect.contains(seat)));
    for seat in seats {
        let waits = physical_waits(&game.players[seat].hand, &game.players[seat].melds);
        for _ in 0..turns {
            let position = if protect.contains(&seat) {
                game.wall
                    .iter()
                    .position(|tile| !waits.contains(tile))
                    .unwrap()
            } else {
                0
            };
            let tile = game.wall.remove(position);
            game.players[seat].discards.push(Discard {
                tile,
                called: false,
                riichi: false,
            });
        }
    }
    game.validate().unwrap();
}

fn won_score(game: &Game) -> &Score {
    match &game.phase {
        Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Win { score, .. },
            ..
        }) => score,
        _ => panic!("和了で局が終了している必要がある"),
    }
}

#[test]
fn yaku_less_completion_still_causes_temporary_furiten() {
    // 開いた234pに、234m・456s・77m・23s。1sは役なし、4sなら断么・三色です。
    let mut game = fixture(0, ["2p", "23477m34p23456s7z", "1s", "4s"], "2p");
    discard(&mut game, 0, "2p");
    reactions(&mut game, &[(1, Action::Chi([one("3p"), one("4p")]))]);
    discard(&mut game, 1, "7z");
    reactions(&mut game, &[]);
    discard(&mut game, 2, "1s");
    assert!(!game.legal_actions(1).contains(&Action::Ron));
    reactions(&mut game, &[]);
    assert!(game.is_furiten(1), "役のない側の和了形も同巡の見逃しに含む");
    discard(&mut game, 3, "4s");
    assert!(!game.legal_actions(1).contains(&Action::Ron));
}

#[test]
fn last_discard_after_rinshan_can_award_houtei() {
    let mut game = fixture(0, ["9999m23455p678s23z", "123456m23455p67s", "", ""], "3z");
    dead_draw(&mut game, "8s");
    shorten_wall(&mut game, 1, &[1]);
    game.apply(0, Action::ClosedKan(one("9m"))).unwrap();
    assert_eq!(game.wall_remaining(), 0);
    assert_eq!(game.last_draw, Some((0, one("8s"), true)));
    discard(&mut game, 0, "8s");
    reactions(&mut game, &[(1, Action::Ron)]);
    assert!(won_score(&game)
        .yaku
        .iter()
        .any(|yaku| yaku.name == "河底撈魚"));
}

#[test]
fn riichi_is_deferred_until_declaration_tile_survives() {
    let mut game = fixture(0, ["123456m234p5567s7z", "", "", ""], "7z");
    let index = game.players[0]
        .hand
        .iter()
        .position(|&tile| tile == one("7z"))
        .unwrap();
    assert!(game.legal_actions(0).contains(&Action::Riichi(index)));
    game.apply(0, Action::Riichi(index)).unwrap();
    assert_eq!(game.riichi_pot(), 0);
    assert!(!game.players[0].riichi);
    game.apply(1, Action::Pass).unwrap();
    game.apply(2, Action::Pass).unwrap();
    assert_eq!(game.riichi_pot(), 0);
    game.apply(3, Action::Pass).unwrap();
    assert_eq!(game.riichi_pot(), 1000);
    assert_eq!(game.scores()[0], 24_000);
    assert!(game.players[0].riichi);
    assert!(game.players[0].double_riichi);
    assert!(game.players[0].ippatsu);
}

#[test]
fn ron_on_declaration_tile_cancels_the_riichi_deposit() {
    let mut game = fixture(0, ["123456m2349p5567s", "123456m7899p234s", "", ""], "9p");
    let index = game.players[0]
        .hand
        .iter()
        .position(|&tile| tile == one("9p"))
        .unwrap();
    game.apply(0, Action::Riichi(index)).unwrap();
    reactions(&mut game, &[(1, Action::Ron)]);
    assert_eq!(game.riichi_pot(), 0);
    assert!(!game.players[0].riichi);
    assert!(!game.players[0].discards.last().unwrap().riichi);
    assert_eq!(game.scores(), [24_000, 26_000, 25_000, 25_000]);
}

#[test]
fn calling_the_declaration_tile_establishes_riichi_but_cancels_ippatsu() {
    let mut game = fixture(0, ["123456m234p55678s", "", "888s", ""], "8s");
    let index = game.players[0]
        .hand
        .iter()
        .position(|&tile| tile == one("8s"))
        .unwrap();
    game.apply(0, Action::Riichi(index)).unwrap();
    reactions(&mut game, &[(2, Action::Pon)]);
    assert!(game.players[0].riichi);
    assert!(!game.players[0].ippatsu);
    assert_eq!(game.riichi_pot(), 1000);
    assert_eq!(game.next_actor(), Some(2));
    assert!(game.players[0].discards.last().unwrap().called);
}

#[test]
fn temporary_furiten_expires_on_the_next_draw() {
    let mut game = fixture(0, ["8s", "", "123456m23455p67s", ""], "8s");
    discard(&mut game, 0, "8s");
    assert!(game.legal_actions(2).contains(&Action::Ron));
    reactions(&mut game, &[]);
    assert!(game.players[2].temporary_furiten);
    assert!(!game.players[2].riichi_furiten);
    let drawn = game.last_draw.unwrap().1;
    discard(&mut game, 1, &drawn.short());
    reactions(&mut game, &[]);
    assert_eq!(game.next_actor(), Some(2));
    assert!(!game.players[2].temporary_furiten);
}

#[test]
fn riichi_furiten_survives_the_next_draw_and_discard_is_locked() {
    let mut game = fixture(0, ["123456m234p5567s7z", "8s", "8s", ""], "7z");
    let index = game.players[0]
        .hand
        .iter()
        .position(|&tile| tile == one("7z"))
        .unwrap();
    game.apply(0, Action::Riichi(index)).unwrap();
    reactions(&mut game, &[]);
    discard(&mut game, 1, "8s");
    assert!(game.legal_actions(0).contains(&Action::Ron));
    reactions(&mut game, &[]);
    assert!(game.players[0].riichi_furiten);
    discard(&mut game, 2, "8s");
    assert!(!game.legal_actions(0).contains(&Action::Ron));
    reactions(&mut game, &[]);
    let drawn = game.last_draw.unwrap().1;
    discard(&mut game, 3, &drawn.short());
    reactions(&mut game, &[]);
    assert_eq!(game.next_actor(), Some(0));
    assert!(game.is_furiten(0));
    let drawn = game.last_draw.unwrap().1;
    for action in game.legal_actions(0) {
        match action {
            Action::Discard(index) => assert_eq!(game.players[0].hand[index], drawn),
            Action::Tsumo => {}
            unexpected => panic!("立直後には許可されない操作: {unexpected:?}"),
        }
    }
}

#[test]
fn ron_has_priority_over_pon_and_chi() {
    let mut game = fixture(0, ["8s", "67s", "888s", "123456m23455p67s"], "8s");
    discard(&mut game, 0, "8s");
    reactions(
        &mut game,
        &[
            (1, Action::Chi([one("6s"), one("7s")])),
            (2, Action::Pon),
            (3, Action::Ron),
        ],
    );
    assert!(matches!(
        &game.phase,
        Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Win {
                winner: 3,
                from: Some(0),
                ..
            },
            ..
        })
    ));
    assert!(game.players[1].melds.is_empty());
    assert!(game.players[2].melds.is_empty());
    assert!(!game.players[0].discards.last().unwrap().called);
}

#[test]
fn head_bump_uses_distance_from_the_discarder() {
    let ready = "123456m23455p67s";
    let mut game = fixture(0, ["8s", ready, ready, ""], "8s");
    discard(&mut game, 0, "8s");
    reactions(&mut game, &[(1, Action::Ron), (2, Action::Ron)]);
    assert!(matches!(
        &game.phase,
        Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Win {
                winner: 1,
                from: Some(0),
                ..
            },
            ..
        })
    ));
    assert_eq!(game.scores()[2], INITIAL_POINTS);
}

#[test]
fn pon_has_priority_over_chi_and_does_not_create_a_tsumo() {
    let mut game = fixture(0, ["8s", "67s", "888s", ""], "8s");
    discard(&mut game, 0, "8s");
    reactions(
        &mut game,
        &[(1, Action::Chi([one("6s"), one("7s")])), (2, Action::Pon)],
    );
    assert_eq!(game.next_actor(), Some(2));
    assert_eq!(game.players[2].melds[0].kind, MeldKind::Pon);
    assert_eq!(game.players[2].hand.len(), 11);
    assert!(game.last_draw.is_none());
    assert!(!game.legal_actions(2).contains(&Action::Tsumo));
    let forbidden = game.players[2]
        .hand
        .iter()
        .position(|&tile| tile == one("8s"))
        .unwrap();
    assert!(!game.legal_actions(2).contains(&Action::Discard(forbidden)));
    game.validate().unwrap();
}

#[test]
fn chi_is_only_from_the_left_and_forbids_both_kuikae_tiles() {
    let mut game = fixture(0, ["1m", "1234m", "23m", ""], "1m");
    discard(&mut game, 0, "1m");
    let chi = Action::Chi([one("2m"), one("3m")]);
    assert!(game.legal_actions(1).contains(&chi));
    assert!(!game.legal_actions(2).contains(&chi));
    reactions(&mut game, &[(1, chi)]);
    for forbidden in ["1m", "4m"] {
        let index = game.players[1]
            .hand
            .iter()
            .position(|&tile| tile == one(forbidden))
            .unwrap();
        let before = game.clone();
        assert!(game.apply(1, Action::Discard(index)).is_err());
        assert_eq!(game, before);
    }
    assert!(game
        .legal_actions(1)
        .iter()
        .any(|action| matches!(action, Action::Discard(_))));
}

#[test]
fn open_kan_draws_from_the_dead_wall_and_preserves_inventory() {
    let mut game = fixture(0, ["5z", "555z", "", ""], "5z");
    dead_draw(&mut game, "8s");
    let before = game.wall.len();
    discard(&mut game, 0, "5z");
    reactions(&mut game, &[(1, Action::OpenKan)]);
    assert_eq!(game.last_draw, Some((1, one("8s"), true)));
    assert_eq!(game.wall.len(), before - 1);
    assert_eq!(game.dead_wall.len(), 14);
    assert_eq!(game.kan_count, 1);
    assert_eq!(game.dora_indicators().len(), 2);
    assert_eq!(game.players[1].melds[0].kind, MeldKind::OpenKan);
    assert_eq!(game.players[1].melds[0].tiles.len(), 4);
    game.validate().unwrap();
}

#[test]
fn rinshan_does_not_combine_with_haitei_when_the_live_wall_becomes_empty() {
    let mut game = fixture(0, ["1111m2234p678s555z", "", "", ""], "2p");
    dead_draw(&mut game, "2p");
    shorten_wall(&mut game, 1, &[]);
    game.apply(0, Action::ClosedKan(one("1m"))).unwrap();
    assert!(game.players[0].is_closed());
    assert_eq!(game.wall_remaining(), 0);
    game.apply(0, Action::Tsumo).unwrap();
    let score = won_score(&game);
    assert!(score.yaku.iter().any(|yaku| yaku.name == "嶺上開花"));
    assert!(!score.yaku.iter().any(|yaku| yaku.name == "海底摸月"));
}

#[test]
fn dealer_tsumo_rounds_each_payment_and_continues_the_dealership() {
    let mut game = fixture(0, ["123456m23455p678s", "", "", ""], "6s");
    game.apply(0, Action::Tsumo).unwrap();
    assert_eq!((won_score(&game).han, won_score(&game).fu), (2, 20));
    assert_eq!(game.scores(), [27_100, 24_300, 24_300, 24_300]);
    game.apply(0, Action::NextRound).unwrap();
    assert_eq!((game.round(), game.dealer(), game.honba()), (0, 0, 1));
    assert_eq!(game.next_actor(), Some(0));
    game.validate().unwrap();
}

#[test]
fn nondealer_tsumo_includes_honba_and_the_riichi_pot() {
    let mut game = fixture(1, ["", "123456m23455p678s", "", ""], "6s");
    game.honba = 2;
    contracts::transfer_points(&mut game.accounts, 2, 4, 1000).unwrap();
    game.validate().unwrap();
    game.apply(1, Action::Tsumo).unwrap();
    assert_eq!(game.scores(), [24_100, 28_100, 23_400, 24_400]);
    assert_eq!(game.riichi_pot(), 0);
    game.apply(0, Action::NextRound).unwrap();
    assert_eq!((game.round(), game.dealer(), game.honba()), (1, 1, 0));
    game.validate().unwrap();
}

#[test]
fn exhaustive_draw_with_two_ready_players_transfers_fifteen_hundred_each() {
    let not_ready = "147m258p369s1234z";
    let mut game = fixture(
        0,
        [
            "123456m234p5567s7z",
            "67789m123p456s22z",
            not_ready,
            not_ready,
        ],
        "7z",
    );
    shorten_wall(&mut game, 1, &[]);
    discard(&mut game, 0, "7z");
    reactions(&mut game, &[]);
    let drawn = game.last_draw.unwrap().1;
    discard(&mut game, 1, &drawn.short());
    reactions(&mut game, &[]);
    assert!(matches!(
        game.phase,
        Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Draw {
                tenpai: [true, true, false, false]
            },
            ..
        })
    ));
    assert_eq!(game.scores(), [26_500, 26_500, 23_500, 23_500]);
    game.apply(0, Action::NextRound).unwrap();
    assert_eq!((game.round(), game.dealer(), game.honba()), (0, 0, 1));
}

#[test]
fn exhaustive_draw_rotates_a_not_ready_dealer_but_keeps_honba() {
    let mut game = fixture(
        0,
        [
            "147m258p369s12347z",
            "67789m123p456s22z",
            "123456m234p5567s",
            "147m258p369s1234z",
        ],
        "7z",
    );
    shorten_wall(&mut game, 1, &[]);
    discard(&mut game, 0, "7z");
    reactions(&mut game, &[]);
    let drawn = game.last_draw.unwrap().1;
    discard(&mut game, 1, &drawn.short());
    reactions(&mut game, &[]);
    assert!(matches!(
        game.phase,
        Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Draw {
                tenpai: [false, true, true, false]
            },
            ..
        })
    ));
    game.apply(0, Action::NextRound).unwrap();
    assert_eq!((game.round(), game.dealer(), game.honba()), (1, 1, 1));
}
