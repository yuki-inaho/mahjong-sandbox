//! 和了形の列挙、待ち牌、向聴数です。役の有無とは分離しています。

use crate::tile::{counts, Tile};

pub(crate) const ORPHANS: [usize; 13] = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GroupKind {
    Sequence,
    Triplet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Group {
    pub kind: GroupKind,
    pub tile: Tile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Shape {
    pub pair: Tile,
    pub groups: Vec<Group>,
}

fn valid_counts(tiles: &[Tile]) -> Option<[u8; 34]> {
    let counts = counts(tiles);
    counts.iter().all(|&n| n <= 4).then_some(counts)
}

pub(crate) fn seven_pairs(counts: &[u8; 34]) -> bool {
    counts.iter().filter(|&&n| n == 2).count() == 7 && counts.iter().all(|&n| n == 0 || n == 2)
}

pub(crate) fn thirteen_orphans(counts: &[u8; 34]) -> bool {
    ORPHANS.iter().all(|&i| counts[i] >= 1)
        && ORPHANS.iter().filter(|&&i| counts[i] == 2).count() == 1
        && counts.iter().map(|&n| usize::from(n)).sum::<usize>() == 14
}

/// 役がなくても、牌の形が完成していればtrueです。
pub fn is_complete(tiles: &[Tile], open_melds: usize) -> bool {
    if open_melds > 4 || tiles.len() != 14 - 3 * open_melds {
        return false;
    }
    let Some(mut counts) = valid_counts(tiles) else {
        return false;
    };
    if open_melds == 0 && (seven_pairs(&counts) || thirteen_orphans(&counts)) {
        return true;
    }
    for pair in 0..34 {
        if counts[pair] >= 2 {
            counts[pair] -= 2;
            let completed = can_form_groups(&mut counts, 4 - open_melds);
            counts[pair] += 2;
            if completed {
                return true;
            }
        }
    }
    false
}

fn can_form_groups(counts: &mut [u8; 34], remaining: usize) -> bool {
    let Some(i) = counts.iter().position(|&count| count > 0) else {
        return remaining == 0;
    };
    if remaining == 0 {
        return false;
    }
    if counts[i] >= 3 {
        counts[i] -= 3;
        let completed = can_form_groups(counts, remaining - 1);
        counts[i] += 3;
        if completed {
            return true;
        }
    }
    if i < 27 && i % 9 <= 6 && counts[i + 1] > 0 && counts[i + 2] > 0 {
        for count in &mut counts[i..=i + 2] {
            *count -= 1;
        }
        let completed = can_form_groups(counts, remaining - 1);
        for count in &mut counts[i..=i + 2] {
            *count += 1;
        }
        return completed;
    }
    false
}

pub(crate) fn standard_shapes(tiles: &[Tile], open_melds: usize) -> Vec<Shape> {
    if open_melds > 4 || tiles.len() != 14 - 3 * open_melds {
        return Vec::new();
    }
    let Some(mut counts) = valid_counts(tiles) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for pair in 0..34 {
        if counts[pair] >= 2 {
            counts[pair] -= 2;
            enumerate_groups(
                &mut counts,
                4 - open_melds,
                Tile::new(pair as u8).unwrap(),
                &mut Vec::new(),
                &mut output,
            );
            counts[pair] += 2;
        }
    }
    output
}

fn enumerate_groups(
    counts: &mut [u8; 34],
    remaining: usize,
    pair: Tile,
    groups: &mut Vec<Group>,
    output: &mut Vec<Shape>,
) {
    let Some(i) = counts.iter().position(|&count| count > 0) else {
        if remaining == 0 {
            output.push(Shape {
                pair,
                groups: groups.clone(),
            });
        }
        return;
    };
    if remaining == 0 {
        return;
    }
    let tile = Tile::new(i as u8).unwrap();
    if counts[i] >= 3 {
        counts[i] -= 3;
        groups.push(Group {
            kind: GroupKind::Triplet,
            tile,
        });
        enumerate_groups(counts, remaining - 1, pair, groups, output);
        groups.pop();
        counts[i] += 3;
    }
    if i < 27 && i % 9 <= 6 && counts[i + 1] > 0 && counts[i + 2] > 0 {
        for count in &mut counts[i..=i + 2] {
            *count -= 1;
        }
        groups.push(Group {
            kind: GroupKind::Sequence,
            tile,
        });
        enumerate_groups(counts, remaining - 1, pair, groups, output);
        groups.pop();
        for count in &mut counts[i..=i + 2] {
            *count += 1;
        }
    }
}

/// 有効な13枚相当の牌姿から、形を完成させる牌を返します。
/// 副露牌の実体は受け取らないため、その中の4枚使用までは判定しません。
pub fn winning_tiles(tiles: &[Tile], open_melds: usize) -> Vec<Tile> {
    if open_melds > 4 || tiles.len() != 13 - 3 * open_melds {
        return Vec::new();
    }
    let Some(counts) = valid_counts(tiles) else {
        return Vec::new();
    };
    let mut candidate = tiles.to_vec();
    let mut output = Vec::new();
    for (id, &count) in counts.iter().enumerate() {
        if count == 4 {
            continue;
        }
        let tile = Tile::new(id as u8).unwrap();
        candidate.push(tile);
        if is_complete(&candidate, open_melds) {
            output.push(tile);
        }
        candidate.pop();
    }
    output
}

/// -1は完成、0は聴牌です。不正な枚数や5枚以上の同牌には8を返します。
/// 対局状態を変更しない、CPUにも再利用できる純粋関数です。
pub fn shanten(tiles: &[Tile], open_melds: usize) -> i8 {
    if open_melds > 4 || tiles.len() > 14 - 3 * open_melds {
        return 8;
    }
    let Some(mut counts) = valid_counts(tiles) else {
        return 8;
    };
    let mut best = 8;
    search_shanten(&mut counts, 0, open_melds as i8, 0, 0, &mut best);
    if open_melds == 0 {
        let pairs = counts.iter().filter(|&&n| n >= 2).count() as i8;
        let distinct = counts.iter().filter(|&&n| n > 0).count() as i8;
        best = best.min(6 - pairs + (7 - distinct).max(0));
        let orphan_distinct = ORPHANS.iter().filter(|&&i| counts[i] > 0).count() as i8;
        let orphan_pair = i8::from(ORPHANS.iter().any(|&i| counts[i] >= 2));
        best = best.min(13 - orphan_distinct - orphan_pair);
    }
    // 同じ牌を4枚とも使った単騎など、5枚目を必要とする形は聴牌ではありません。
    if best == 0
        && tiles.len() == 13 - 3 * open_melds
        && winning_tiles(tiles, open_melds).is_empty()
    {
        1
    } else {
        best
    }
}

fn search_shanten(
    counts: &mut [u8; 34],
    start: usize,
    melds: i8,
    partials: i8,
    head: i8,
    best: &mut i8,
) {
    let Some(i) = (start..34).find(|&i| counts[i] > 0) else {
        *best = (*best).min(8 - 2 * melds - partials.min(4 - melds) - head);
        return;
    };
    if *best == -1 {
        return;
    }
    if melds < 4 {
        if counts[i] >= 3 {
            counts[i] -= 3;
            search_shanten(counts, i, melds + 1, partials, head, best);
            counts[i] += 3;
        }
        if i < 27 && i % 9 <= 6 && counts[i + 1] > 0 && counts[i + 2] > 0 {
            for count in &mut counts[i..=i + 2] {
                *count -= 1;
            }
            search_shanten(counts, i, melds + 1, partials, head, best);
            for count in &mut counts[i..=i + 2] {
                *count += 1;
            }
        }
    }
    if counts[i] >= 2 {
        counts[i] -= 2;
        if head == 0 {
            search_shanten(counts, i, melds, partials, 1, best);
        }
        if melds + partials < 4 {
            search_shanten(counts, i, melds, partials + 1, head, best);
        }
        counts[i] += 2;
    }
    if melds + partials < 4 && i < 27 {
        for gap in 1..=2 {
            if i % 9 + gap < 9 && counts[i + gap] > 0 {
                counts[i] -= 1;
                counts[i + gap] -= 1;
                search_shanten(counts, i, melds, partials + 1, head, best);
                counts[i] += 1;
                counts[i + gap] += 1;
            }
        }
    }
    counts[i] -= 1;
    search_shanten(counts, i, melds, partials, head, best);
    counts[i] += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::parse_tiles;

    #[test]
    fn recognizes_standard_seven_pairs_and_orphans() {
        for input in [
            "123456789m12344p",
            "1122m3344p5566s77z",
            "19m19p19s12345677z",
        ] {
            let tiles = parse_tiles(input).unwrap();
            assert!(is_complete(&tiles, 0), "{input}");
            assert_eq!(shanten(&tiles, 0), -1, "{input}");
        }
    }

    #[test]
    fn quads_are_not_two_pairs() {
        let tiles = parse_tiles("1111m2233p4455s66z").unwrap();
        assert!(!is_complete(&tiles, 0));
        assert!(shanten(&tiles, 0) >= 0);
    }

    #[test]
    fn validates_size_multiplicity_and_open_meld_count() {
        let mut tiles = parse_tiles("111123m456p789s11z").unwrap();
        tiles[5] = Tile::new(0).unwrap();
        assert!(!is_complete(&tiles, 0));
        assert_eq!(shanten(&tiles, 0), 8);
        assert!(!is_complete(&[], usize::MAX));
        assert!(winning_tiles(&[], usize::MAX).is_empty());
        assert_eq!(shanten(&[], usize::MAX), 8);
        assert!(!is_complete(&parse_tiles("123m").unwrap(), 0));
    }

    #[test]
    fn does_not_join_sequences_across_suits() {
        assert!(!is_complete(&parse_tiles("89m1p234567s11122z").unwrap(), 0));
    }

    #[test]
    fn enumerates_both_waits_and_thirteen_orphan_waits() {
        let tiles = parse_tiles("123456m234p55s67s").unwrap();
        let waits = winning_tiles(&tiles, 0);
        assert_eq!(waits, parse_tiles("58s").unwrap());
        assert_eq!(shanten(&tiles, 0), 0);
        assert_eq!(
            winning_tiles(&parse_tiles("19m19p19s1234567z").unwrap(), 0).len(),
            13
        );
    }

    #[test]
    fn handles_four_open_groups_and_pair_wait() {
        assert!(is_complete(&parse_tiles("11z").unwrap(), 4));
        assert_eq!(
            winning_tiles(&parse_tiles("1z").unwrap(), 4),
            parse_tiles("1z").unwrap()
        );
        assert_eq!(shanten(&parse_tiles("1z").unwrap(), 4), 0);
    }

    #[test]
    fn keeps_ambiguous_decompositions_for_scoring() {
        let shapes = standard_shapes(&parse_tiles("111222333m44455p").unwrap(), 0);
        assert!(shapes.len() >= 2);
    }

    #[test]
    fn a_wait_requiring_a_fifth_tile_is_not_tenpai() {
        let tiles = parse_tiles("1111m234p567789s").unwrap();
        assert!(winning_tiles(&tiles, 0).is_empty());
        assert_eq!(shanten(&tiles, 0), 1);
    }
}
