//! 和了した手牌を、役・符・基本点へ変換します。

use crate::hand::{seven_pairs, standard_shapes, thirteen_orphans, GroupKind, Shape};
use crate::rule_constants as rules;
use crate::tile::{counts, Tile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeldKind {
    Chi,
    Pon,
    OpenKan,
    ClosedKan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Meld {
    pub kind: MeldKind,
    pub tiles: Vec<Tile>,
    pub from: Option<usize>,
}

impl Meld {
    pub fn is_open(&self) -> bool {
        self.kind != MeldKind::ClosedKan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinContext {
    pub win_tile: Tile,
    pub tsumo: bool,
    pub seat_wind: Tile,
    pub round_wind: Tile,
    pub riichi: bool,
    pub double_riichi: bool,
    pub ippatsu: bool,
    pub last_tile: bool,
    pub rinshan: bool,
    pub chankan: bool,
    pub dora_indicators: Vec<Tile>,
    pub ura_indicators: Vec<Tile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Yaku {
    pub name: &'static str,
    pub han: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Score {
    pub han: u8,
    pub fu: u16,
    pub yakuman: u8,
    pub base_points: u32,
    pub yaku: Vec<Yaku>,
    pub dora: u8,
    pub limit_name: Option<&'static str>,
}

/// 事前条件を検査し、役のある合法な和了形のうち最大の点数を返します。
/// `concealed` は和了牌を含みます。ドラだけの和了、不正な副露、5枚目の同牌は拒否します。
pub fn evaluate(concealed: &[Tile], melds: &[Meld], ctx: &WinContext) -> Option<Score> {
    if !valid_input(concealed, melds, ctx) {
        return None;
    }
    let mut all_tiles = concealed.to_vec();
    for meld in melds {
        all_tiles.extend_from_slice(&meld.tiles);
    }
    let closed = melds.iter().all(|meld| !meld.is_open());
    let concealed_counts = counts(concealed);
    let mut candidates = Vec::new();
    if melds.is_empty() && thirteen_orphans(&concealed_counts) {
        let mut before_win = concealed_counts;
        before_win[usize::from(ctx.win_tile.id())] -= 1;
        let thirteen_sided = crate::hand::ORPHANS.iter().all(|&id| before_win[id] == 1);
        let multiplier = if rules::DOUBLE_YAKUMAN && thirteen_sided {
            2
        } else {
            1
        };
        candidates.push(yakuman_score(vec![Yaku {
            name: "国士無双",
            han: 13 * multiplier,
        }]));
    }
    if melds.is_empty() && seven_pairs(&concealed_counts) {
        let yakuman = tile_yakuman(&all_tiles, melds, ctx);
        if yakuman.is_empty() {
            let mut yaku = common_yaku(&all_tiles, closed, ctx);
            yaku.push(Yaku {
                name: "七対子",
                han: 2,
            });
            candidates.push(ordinary_score(yaku, 25, count_dora(&all_tiles, ctx)));
        } else {
            candidates.push(yakuman_score(yakuman));
        }
    }
    for shape in standard_shapes(concealed, melds.len()) {
        // 和了牌の割当てで待ちの符、平和、三暗刻・四暗刻が変わるため全て比較します。
        if shape.pair == ctx.win_tile {
            if let Some(score) = standard_score(&shape, melds, &all_tiles, ctx, Placement::Pair) {
                candidates.push(score);
            }
        }
        for (index, group) in shape.groups.iter().enumerate() {
            let matches = match group.kind {
                GroupKind::Triplet => group.tile == ctx.win_tile,
                GroupKind::Sequence => {
                    ctx.win_tile.id() >= group.tile.id() && ctx.win_tile.id() <= group.tile.id() + 2
                }
            };
            if matches {
                if let Some(score) =
                    standard_score(&shape, melds, &all_tiles, ctx, Placement::Group(index))
                {
                    candidates.push(score);
                }
            }
        }
    }
    candidates
        .into_iter()
        .max_by_key(|score| (score.base_points, score.yakuman, score.han, score.fu))
}

fn valid_input(concealed: &[Tile], melds: &[Meld], ctx: &WinContext) -> bool {
    if melds.len() > 4
        || concealed.len() != 14 - 3 * melds.len()
        || !concealed.contains(&ctx.win_tile)
        || !(27..31).contains(&ctx.seat_wind.id())
        || !(27..31).contains(&ctx.round_wind.id())
        || ctx.dora_indicators.len() > rules::MAX_KANS + 1
        || ctx.ura_indicators.len() > rules::MAX_KANS + 1
        || (ctx.double_riichi && !ctx.riichi)
        || (ctx.ippatsu && !ctx.riichi)
        || (ctx.riichi && melds.iter().any(Meld::is_open))
        || (ctx.rinshan && (!ctx.tsumo || ctx.last_tile))
        || (ctx.rinshan
            && !melds
                .iter()
                .any(|m| matches!(m.kind, MeldKind::OpenKan | MeldKind::ClosedKan)))
        || (ctx.chankan && (ctx.tsumo || !rules::ALLOW_ADDED_KAN))
    {
        return false;
    }
    let mut physical = counts(concealed);
    for meld in melds {
        if !valid_meld(meld) {
            return false;
        }
        for tile in &meld.tiles {
            let count = &mut physical[usize::from(tile.id())];
            *count = count.saturating_add(1);
        }
    }
    physical
        .iter()
        .all(|&count| count <= rules::COPIES_PER_TILE)
}

fn valid_meld(meld: &Meld) -> bool {
    let expected_len = match meld.kind {
        MeldKind::Chi | MeldKind::Pon => 3,
        MeldKind::OpenKan | MeldKind::ClosedKan => 4,
    };
    if meld.tiles.len() != expected_len
        || (meld.is_open() && !matches!(meld.from, Some(seat) if seat < rules::PLAYER_COUNT))
        || (!meld.is_open() && meld.from.is_some())
    {
        return false;
    }
    let mut tiles = meld.tiles.clone();
    tiles.sort_unstable();
    if meld.kind == MeldKind::Chi {
        !tiles[0].is_honor()
            && tiles[0].rank() <= 7
            && tiles[1].id() == tiles[0].id() + 1
            && tiles[2].id() == tiles[0].id() + 2
    } else {
        tiles.iter().all(|&tile| tile == tiles[0])
    }
}

#[derive(Clone, Copy)]
enum Placement {
    Pair,
    Group(usize),
}

#[derive(Clone, Copy)]
struct ScoringGroup {
    tile: Tile,
    sequence: bool,
    open: bool,
    kan: bool,
}

fn scoring_groups(
    shape: &Shape,
    melds: &[Meld],
    ctx: &WinContext,
    target: Placement,
) -> Vec<ScoringGroup> {
    let mut groups: Vec<ScoringGroup> = shape
        .groups
        .iter()
        .enumerate()
        .map(|(i, group)| ScoringGroup {
            tile: group.tile,
            sequence: group.kind == GroupKind::Sequence,
            open: !ctx.tsumo
                && matches!(target, Placement::Group(index) if index == i)
                && group.kind == GroupKind::Triplet,
            kan: false,
        })
        .collect();
    for meld in melds {
        groups.push(ScoringGroup {
            tile: *meld.tiles.iter().min().expect("副露は検査済み"),
            sequence: meld.kind == MeldKind::Chi,
            open: meld.is_open(),
            kan: matches!(meld.kind, MeldKind::OpenKan | MeldKind::ClosedKan),
        });
    }
    groups
}

fn standard_score(
    shape: &Shape,
    melds: &[Meld],
    all_tiles: &[Tile],
    ctx: &WinContext,
    target: Placement,
) -> Option<Score> {
    let closed = melds.iter().all(|meld| !meld.is_open());
    let groups = scoring_groups(shape, melds, ctx, target);
    let concealed_triplets = groups
        .iter()
        .filter(|group| !group.sequence && !group.open)
        .count();
    let kans = groups.iter().filter(|group| group.kan).count();
    let dragons = groups
        .iter()
        .filter(|group| !group.sequence && group.tile.id() >= 31)
        .count();
    let winds = groups
        .iter()
        .filter(|group| !group.sequence && (27..31).contains(&group.tile.id()))
        .count();
    let mut yakuman = tile_yakuman(all_tiles, melds, ctx);
    if concealed_triplets == 4 {
        yakuman.push(Yaku {
            name: "四暗刻",
            han: if rules::DOUBLE_YAKUMAN && matches!(target, Placement::Pair) {
                26
            } else {
                13
            },
        });
    }
    if kans == 4 {
        yakuman.push(Yaku {
            name: "四槓子",
            han: 13,
        });
    }
    if dragons == 3 {
        yakuman.push(Yaku {
            name: "大三元",
            han: 13,
        });
    }
    if winds == 4 {
        yakuman.push(Yaku {
            name: "大四喜",
            han: if rules::DOUBLE_YAKUMAN { 26 } else { 13 },
        });
    } else if winds == 3 && (27..31).contains(&shape.pair.id()) {
        yakuman.push(Yaku {
            name: "小四喜",
            han: 13,
        });
    }
    if !yakuman.is_empty() {
        return Some(yakuman_score(yakuman));
    }

    let pair_fu = pair_fu(shape.pair, ctx);
    let wait_fu = wait_fu(shape, ctx, target);
    let pinfu = closed && groups.iter().all(|group| group.sequence) && pair_fu == 0 && wait_fu == 0;
    let mut yaku = common_yaku(all_tiles, closed, ctx);
    if pinfu {
        yaku.push(Yaku {
            name: "平和",
            han: 1,
        });
    }
    for group in groups.iter().filter(|group| !group.sequence) {
        match group.tile.id() {
            31 => yaku.push(Yaku {
                name: "役牌・白",
                han: 1,
            }),
            32 => yaku.push(Yaku {
                name: "役牌・發",
                han: 1,
            }),
            33 => yaku.push(Yaku {
                name: "役牌・中",
                han: 1,
            }),
            _ => {}
        }
        if group.tile == ctx.seat_wind {
            yaku.push(Yaku {
                name: "役牌・自風",
                han: 1,
            });
        }
        if group.tile == ctx.round_wind {
            yaku.push(Yaku {
                name: "役牌・場風",
                han: 1,
            });
        }
    }
    if concealed_triplets == 3 {
        yaku.push(Yaku {
            name: "三暗刻",
            han: 2,
        });
    }
    if kans == 3 {
        yaku.push(Yaku {
            name: "三槓子",
            han: 2,
        });
    }
    if groups.iter().all(|group| !group.sequence) {
        yaku.push(Yaku {
            name: "対々和",
            han: 2,
        });
    }
    if dragons == 2 && shape.pair.id() >= 31 {
        yaku.push(Yaku {
            name: "小三元",
            han: 2,
        });
    }
    sequence_yaku(&groups, shape.pair, closed, &mut yaku);
    if yaku.is_empty() {
        return None;
    }

    let mut fu = 20 + pair_fu + wait_fu;
    if closed && !ctx.tsumo {
        fu += 10;
    }
    if ctx.tsumo && !pinfu {
        fu += 2;
    }
    for group in groups.iter().filter(|group| !group.sequence) {
        let mut group_fu = if group.kan { 8 } else { 2 };
        if group.tile.is_terminal_or_honor() {
            group_fu *= 2;
        }
        if !group.open {
            group_fu *= 2;
        }
        fu += group_fu;
    }
    // 副露した順子形のロンは、加符なしでも30符とします。
    if fu == 20 && !pinfu {
        fu = 30;
    }
    fu = fu.div_ceil(10) * 10;
    Some(ordinary_score(yaku, fu, count_dora(all_tiles, ctx)))
}

fn pair_fu(pair: Tile, ctx: &WinContext) -> u16 {
    if pair.id() >= 31 {
        2
    } else if pair == ctx.seat_wind && pair == ctx.round_wind {
        rules::DOUBLE_WIND_PAIR_FU
    } else if pair == ctx.seat_wind || pair == ctx.round_wind {
        2
    } else {
        0
    }
}

fn wait_fu(shape: &Shape, ctx: &WinContext, target: Placement) -> u16 {
    match target {
        Placement::Pair => 2,
        Placement::Group(i) => {
            let group = shape.groups[i];
            if group.kind == GroupKind::Triplet {
                0
            } else {
                let start = group.tile.rank();
                let won = ctx.win_tile.rank();
                if won == start + 1 || (start == 1 && won == 3) || (start == 7 && won == 7) {
                    2
                } else {
                    0
                }
            }
        }
    }
}

fn sequence_yaku(groups: &[ScoringGroup], pair: Tile, closed: bool, yaku: &mut Vec<Yaku>) {
    let mut sequence_counts = [0_u8; 27];
    let mut triplets = [false; 34];
    for group in groups {
        if group.sequence {
            sequence_counts[usize::from(group.tile.id())] += 1;
        } else {
            triplets[usize::from(group.tile.id())] = true;
        }
    }
    if closed {
        let identical_pairs: u8 = sequence_counts.iter().map(|count| count / 2).sum();
        if identical_pairs >= 2 {
            yaku.push(Yaku {
                name: "二盃口",
                han: 3,
            });
        } else if identical_pairs == 1 {
            yaku.push(Yaku {
                name: "一盃口",
                han: 1,
            });
        }
    }
    if (0..7).any(|rank| (0..3).all(|suit| sequence_counts[suit * 9 + rank] > 0)) {
        yaku.push(Yaku {
            name: "三色同順",
            han: if closed { 2 } else { 1 },
        });
    }
    if (0..9).any(|rank| (0..3).all(|suit| triplets[suit * 9 + rank])) {
        yaku.push(Yaku {
            name: "三色同刻",
            han: 2,
        });
    }
    if (0..3).any(|suit| {
        [0, 3, 6]
            .iter()
            .all(|&rank| sequence_counts[suit * 9 + rank] > 0)
    }) {
        yaku.push(Yaku {
            name: "一気通貫",
            han: if closed { 2 } else { 1 },
        });
    }
    let has_sequence = groups.iter().any(|group| group.sequence);
    let every_group_outside = groups.iter().all(|group| {
        if group.sequence {
            group.tile.rank() == 1 || group.tile.rank() == 7
        } else {
            group.tile.is_terminal_or_honor()
        }
    });
    if has_sequence && every_group_outside && pair.is_terminal_or_honor() {
        if pair.is_honor() || groups.iter().any(|group| group.tile.is_honor()) {
            yaku.push(Yaku {
                name: "混全帯么九",
                han: if closed { 2 } else { 1 },
            });
        } else {
            yaku.push(Yaku {
                name: "純全帯么九",
                han: if closed { 3 } else { 2 },
            });
        }
    }
}

fn common_yaku(tiles: &[Tile], closed: bool, ctx: &WinContext) -> Vec<Yaku> {
    let mut yaku = Vec::new();
    if ctx.riichi {
        yaku.push(if ctx.double_riichi {
            Yaku {
                name: "ダブル立直",
                han: 2,
            }
        } else {
            Yaku {
                name: "立直",
                han: 1,
            }
        });
    }
    if ctx.ippatsu {
        yaku.push(Yaku {
            name: "一発",
            han: 1,
        });
    }
    if closed && ctx.tsumo {
        yaku.push(Yaku {
            name: "門前清自摸和",
            han: 1,
        });
    }
    if ctx.last_tile {
        yaku.push(Yaku {
            name: if ctx.tsumo {
                "海底摸月"
            } else {
                "河底撈魚"
            },
            han: 1,
        });
    }
    if ctx.rinshan {
        yaku.push(Yaku {
            name: "嶺上開花",
            han: 1,
        });
    }
    if ctx.chankan {
        yaku.push(Yaku {
            name: "搶槓",
            han: 1,
        });
    }
    if (closed || rules::OPEN_TANYAO) && tiles.iter().all(|tile| !tile.is_terminal_or_honor()) {
        yaku.push(Yaku {
            name: "断么九",
            han: 1,
        });
    }
    if tiles.iter().all(|tile| tile.is_terminal_or_honor()) {
        yaku.push(Yaku {
            name: "混老頭",
            han: 2,
        });
    }
    let mut suits = [false; 3];
    let mut honors = false;
    for tile in tiles {
        if let Some(suit) = tile.suit() {
            suits[usize::from(suit)] = true;
        } else {
            honors = true;
        }
    }
    if suits.iter().filter(|&&present| present).count() == 1 {
        if honors {
            yaku.push(Yaku {
                name: "混一色",
                han: if closed { 3 } else { 2 },
            });
        } else {
            yaku.push(Yaku {
                name: "清一色",
                han: if closed { 6 } else { 5 },
            });
        }
    }
    yaku
}

fn tile_yakuman(tiles: &[Tile], melds: &[Meld], ctx: &WinContext) -> Vec<Yaku> {
    let mut yaku = Vec::new();
    if tiles.iter().all(|tile| tile.is_honor()) {
        yaku.push(Yaku {
            name: "字一色",
            han: 13,
        });
    }
    if tiles.iter().all(|tile| tile.is_terminal()) {
        yaku.push(Yaku {
            name: "清老頭",
            han: 13,
        });
    }
    if tiles
        .iter()
        .all(|tile| [19, 20, 21, 23, 25, 32].contains(&tile.id()))
    {
        yaku.push(Yaku {
            name: "緑一色",
            han: 13,
        });
    }
    if melds.is_empty() {
        let counts = counts(tiles);
        for suit in 0..3 {
            let start = suit * 9;
            if counts[start] >= 3
                && counts[start + 8] >= 3
                && counts[start + 1..start + 8].iter().all(|&n| n >= 1)
                && counts[start..start + 9]
                    .iter()
                    .map(|&n| usize::from(n))
                    .sum::<usize>()
                    == 14
            {
                let mut before_win = counts;
                before_win[usize::from(ctx.win_tile.id())] -= 1;
                let pure_wait = before_win[start] == 3
                    && before_win[start + 8] == 3
                    && before_win[start + 1..start + 8].iter().all(|&n| n == 1);
                yaku.push(Yaku {
                    name: "九蓮宝燈",
                    han: if rules::DOUBLE_YAKUMAN && pure_wait {
                        26
                    } else {
                        13
                    },
                });
            }
        }
    }
    yaku
}

fn count_dora(tiles: &[Tile], ctx: &WinContext) -> u8 {
    let ura: &[Tile] = if ctx.riichi { &ctx.ura_indicators } else { &[] };
    ctx.dora_indicators
        .iter()
        .chain(ura.iter())
        .map(|indicator| {
            tiles
                .iter()
                .filter(|&&tile| tile == indicator.dora())
                .count() as u8
        })
        .sum()
}

fn yakuman_score(mut yaku: Vec<Yaku>) -> Score {
    if !rules::COMBINED_YAKUMAN {
        yaku = yaku
            .into_iter()
            .max_by_key(|yaku| yaku.han)
            .into_iter()
            .collect();
    }
    let yakuman = yaku.iter().map(|yaku| yaku.han / 13).sum();
    Score {
        han: 0,
        fu: 0,
        yakuman,
        base_points: 8000 * u32::from(yakuman),
        yaku,
        dora: 0,
        limit_name: Some(if yakuman > 1 {
            "複合役満"
        } else {
            "役満"
        }),
    }
}

fn ordinary_score(yaku: Vec<Yaku>, fu: u16, dora: u8) -> Score {
    let han = yaku.iter().map(|yaku| yaku.han).sum::<u8>() + dora;
    let (base_points, limit_name) = base_points(han, fu);
    Score {
        han,
        fu,
        yakuman: 0,
        base_points,
        yaku,
        dora,
        limit_name,
    }
}

fn base_points(han: u8, fu: u16) -> (u32, Option<&'static str>) {
    if han >= 13 && rules::COUNTED_YAKUMAN {
        (8000, Some("数え役満"))
    } else if han >= 11 {
        (6000, Some("三倍満"))
    } else if han >= 8 {
        (4000, Some("倍満"))
    } else if han >= 6 {
        (3000, Some("跳満"))
    } else if han == 5 {
        (2000, Some("満貫"))
    } else {
        let base = u32::from(fu) * (1_u32 << (han + 2));
        if base >= 2000 || (rules::KIRIAGE_MANGAN && base == 1920) {
            (2000, Some("満貫"))
        } else {
            (base, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::parse_tiles;

    fn context(tile: &str, tsumo: bool) -> WinContext {
        WinContext {
            win_tile: parse_tiles(tile).unwrap()[0],
            tsumo,
            seat_wind: Tile::new(28).unwrap(),
            round_wind: Tile::new(27).unwrap(),
            riichi: false,
            double_riichi: false,
            ippatsu: false,
            last_tile: false,
            rinshan: false,
            chankan: false,
            dora_indicators: Vec::new(),
            ura_indicators: Vec::new(),
        }
    }

    fn score(text: &str, ctx: &WinContext) -> Score {
        evaluate(&parse_tiles(text).unwrap(), &[], ctx).expect("合法な役付き和了")
    }

    fn has(score: &Score, name: &str) -> bool {
        score.yaku.iter().any(|yaku| yaku.name == name)
    }

    fn meld(kind: MeldKind, text: &str) -> Meld {
        Meld {
            kind,
            tiles: parse_tiles(text).unwrap(),
            from: if kind == MeldKind::ClosedKan {
                None
            } else {
                Some(1)
            },
        }
    }

    #[test]
    fn pinfu_tsumo_is_twenty_fu_and_ron_is_thirty() {
        let hand = "123456m23455p678s";
        let tsumo = score(hand, &context("6s", true));
        assert_eq!((tsumo.han, tsumo.fu, tsumo.base_points), (2, 20, 320));
        let ron = score(hand, &context("6s", false));
        assert_eq!((ron.han, ron.fu, ron.base_points), (1, 30, 240));
    }

    #[test]
    fn closed_edge_inside_and_pair_waits_add_fu() {
        for tile in ["3m", "2m", "5p"] {
            let mut ctx = context(tile, false);
            ctx.riichi = true;
            let result = score("123456m23455p678s", &ctx);
            assert_eq!((result.han, result.fu), (1, 40), "{tile}");
            assert!(!has(&result, "平和"));
        }
    }

    #[test]
    fn seven_pairs_uses_fixed_twenty_five_fu() {
        let result = score("1122m3344p5566s77z", &context("7z", false));
        assert_eq!((result.han, result.fu, result.base_points), (2, 25, 400));
        assert!(has(&result, "七対子"));
    }

    #[test]
    fn ambiguous_seven_pairs_chooses_ryanpeikou_shape() {
        let result = score("11223344556677m", &context("7m", false));
        assert!(has(&result, "二盃口"));
        assert!(!has(&result, "七対子"));
        assert_eq!(result.han, 10);
    }

    #[test]
    fn dora_alone_cannot_make_a_yaku() {
        let mut ctx = context("6p", false);
        ctx.dora_indicators = parse_tiles("1m").unwrap();
        assert!(evaluate(
            &parse_tiles("11222m456p789s").unwrap(),
            &[meld(MeldKind::Chi, "123m")],
            &ctx
        )
        .is_none());
    }

    #[test]
    fn open_tanyao_without_other_fu_is_thirty_fu() {
        let result = evaluate(
            &parse_tiles("456m22345p678s").unwrap(),
            &[meld(MeldKind::Chi, "234m")],
            &context("3p", false),
        )
        .unwrap();
        assert_eq!((result.han, result.fu), (1, 30));
        assert!(has(&result, "断么九"));
        assert!(!has(&result, "平和"));
    }

    #[test]
    fn dragon_triplet_adds_yaku_and_concealed_fu() {
        let result = score("123m22456p789s555z", &context("9s", false));
        assert_eq!((result.han, result.fu), (1, 40));
        assert!(has(&result, "役牌・白"));
    }

    #[test]
    fn double_wind_pair_counts_four_fu() {
        let mut ctx = context("6s", false);
        ctx.seat_wind = Tile::new(27).unwrap();
        ctx.riichi = true;
        let result = score("222789m333p456s11z", &ctx);
        assert_eq!(result.fu, 50);
    }

    #[test]
    fn ron_triplet_is_open_for_fu_and_three_concealed_triplets() {
        let mut ctx = context("3s", false);
        ctx.riichi = true;
        let result = score("111456m22255p333s", &ctx);
        assert_eq!(result.fu, 50);
        assert!(!has(&result, "三暗刻"));
        ctx.win_tile = parse_tiles("5p").unwrap()[0];
        assert!(has(&score("111456m22255p333s", &ctx), "三暗刻"));
    }

    #[test]
    fn closed_kan_keeps_menzen_and_adds_thirty_two_fu() {
        let mut ctx = context("2p", true);
        ctx.riichi = true;
        let result = evaluate(
            &parse_tiles("22234p678s555z").unwrap(),
            &[meld(MeldKind::ClosedKan, "1111m")],
            &ctx,
        )
        .unwrap();
        assert!(has(&result, "門前清自摸和"));
        assert_eq!((result.han, result.fu), (3, 70));
    }

    #[test]
    fn kokushi_and_four_concealed_triplets_are_yakuman() {
        let kokushi = score("19m19p19s12345677z", &context("7z", false));
        assert_eq!((kokushi.yakuman, kokushi.base_points), (1, 8000));
        assert!(has(&kokushi, "国士無双"));
        let hand = "111m22255p333s444z";
        assert!(has(&score(hand, &context("3s", true)), "四暗刻"));
        assert!(has(&score(hand, &context("5p", false)), "四暗刻"));
        assert!(!has(&score(hand, &context("3s", false)), "四暗刻"));
    }

    #[test]
    fn yakuman_can_stack_but_special_waits_are_not_doubled() {
        let result = score("11122233344455z", &context("5z", true));
        assert_eq!((result.yakuman, result.base_points), (3, 24000));
        for name in ["大四喜", "字一色", "四暗刻"] {
            assert!(has(&result, name));
        }
    }

    #[test]
    fn recognizes_nine_gates_green_and_big_dragons() {
        for (hand, tile, name) in [
            ("11123455678999m", "5m", "九蓮宝燈"),
            ("22233344466688s", "8s", "緑一色"),
            ("123m55p555666777z", "3m", "大三元"),
        ] {
            assert!(has(&score(hand, &context(tile, false)), name), "{name}");
        }
    }

    #[test]
    fn recognizes_sequence_and_outside_yaku() {
        for (hand, tile, name) in [
            ("123m12355p123789s", "9s", "三色同順"),
            ("123456789m234p55s", "1m", "一気通貫"),
            ("123m11789p111s555z", "3m", "混全帯么九"),
            ("123m11789p111999s", "3m", "純全帯么九"),
            ("1199m1199p1199s11z", "1z", "混老頭"),
            ("111m234p55566677z", "4p", "小三元"),
            ("222456m22255p222s", "6m", "三色同刻"),
        ] {
            assert!(has(&score(hand, &context(tile, false)), name), "{name}");
        }
    }

    #[test]
    fn four_kans_accept_eighteen_physical_tiles() {
        let melds =
            ["1111m", "9999m", "1111p", "9999p"].map(|text| meld(MeldKind::ClosedKan, text));
        let result = evaluate(&parse_tiles("11s").unwrap(), &melds, &context("1s", true)).unwrap();
        assert_eq!(result.yakuman, 3);
        assert!(has(&result, "四槓子"));
    }

    #[test]
    fn malformed_melds_and_fifth_physical_tile_are_rejected() {
        let concealed = parse_tiles("123m456p789s11z").unwrap();
        for bad in [
            meld(MeldKind::Chi, "123z"),
            meld(MeldKind::Pon, "123m"),
            meld(MeldKind::OpenKan, "111m"),
            meld(MeldKind::Pon, "111z"),
        ] {
            assert!(evaluate(&concealed, &[bad], &context("1m", true)).is_none());
        }
    }

    #[test]
    fn inconsistent_context_and_absent_winning_tile_are_rejected() {
        let hand = parse_tiles("123456m23455p678s").unwrap();
        assert!(evaluate(&hand, &[], &context("1z", true)).is_none());
        let mut ctx = context("6s", true);
        ctx.double_riichi = true;
        assert!(evaluate(&hand, &[], &ctx).is_none());
        ctx.double_riichi = false;
        ctx.seat_wind = Tile::new(31).unwrap();
        assert!(evaluate(&hand, &[], &ctx).is_none());
    }

    #[test]
    fn points_follow_declared_limits_without_kiriage() {
        for (han, fu, expected) in [
            (1, 30, 240),
            (2, 40, 640),
            (3, 60, 1920),
            (4, 30, 1920),
            (4, 40, 2000),
            (5, 30, 2000),
            (6, 30, 3000),
            (8, 30, 4000),
            (11, 30, 6000),
            (13, 30, 8000),
        ] {
            assert_eq!(base_points(han, fu).0, expected, "{han}翻{fu}符");
        }
    }

    #[test]
    fn ura_is_only_counted_for_riichi() {
        let mut ctx = context("6s", false);
        ctx.ura_indicators = parse_tiles("4p").unwrap();
        let hand = "123456m23455p678s";
        let without = score(hand, &ctx);
        assert_eq!((without.han, without.dora), (1, 0));
        ctx.riichi = true;
        let with = score(hand, &ctx);
        assert_eq!((with.han, with.dora, with.base_points), (4, 2, 1920));
    }

    #[test]
    fn double_riichi_replaces_riichi_and_ippatsu_combines() {
        let mut ctx = context("6s", true);
        ctx.riichi = true;
        ctx.double_riichi = true;
        ctx.ippatsu = true;
        let result = score("123456m23455p678s", &ctx);
        assert_eq!((result.han, result.fu, result.base_points), (5, 20, 2000));
        assert!(has(&result, "ダブル立直"));
        assert!(!has(&result, "立直"));
    }
}
