//! 34種類の牌と、外部入力を検査する境界です。

use std::fmt;

/// 有効な牌IDだけを保持します。0..8が萬子、9..17が筒子、18..26が索子です。
/// 27..33は東・南・西・北・白・發・中です。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tile(u8);

impl Tile {
    pub fn new(id: u8) -> Option<Self> {
        (usize::from(id) < crate::rule_constants::TILE_KINDS).then_some(Self(id))
    }

    pub const fn id(self) -> u8 {
        self.0
    }

    pub const fn suit(self) -> Option<u8> {
        if self.0 < 27 {
            Some(self.0 / 9)
        } else {
            None
        }
    }

    pub const fn rank(self) -> u8 {
        if self.0 < 27 {
            self.0 % 9 + 1
        } else {
            self.0 - 26
        }
    }

    pub const fn is_honor(self) -> bool {
        self.0 >= 27
    }

    pub const fn is_terminal(self) -> bool {
        self.0 < 27 && (self.0 % 9 == 0 || self.0 % 9 == 8)
    }

    pub const fn is_terminal_or_honor(self) -> bool {
        self.is_honor() || self.is_terminal()
    }

    pub fn short(self) -> String {
        let suit = match self.suit() {
            Some(0) => 'm',
            Some(1) => 'p',
            Some(2) => 's',
            _ => 'z',
        };
        format!("{}{suit}", self.rank())
    }

    pub fn japanese(self) -> String {
        const NUMERALS: [&str; 9] = ["一", "二", "三", "四", "五", "六", "七", "八", "九"];
        const HONORS: [&str; 7] = ["東", "南", "西", "北", "白", "發", "中"];
        match self.suit() {
            Some(suit) => format!(
                "{}{}",
                NUMERALS[usize::from(self.rank() - 1)],
                ["萬", "筒", "索"][usize::from(suit)]
            ),
            None => HONORS[usize::from(self.rank() - 1)].to_owned(),
        }
    }

    /// ドラ表示牌の循環は、形式検証対象の契約関数を使用します。
    pub fn dora(self) -> Self {
        let id = crate::contracts::dora_from_indicator(self.0)
            .expect("Tileは構築時に有効なIDを保証する");
        Self(id)
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.short())
    }
}

/// `123m456p789s11222z` のような牌列を読み込みます。
/// 東南西北白發中の一文字表記も使用できます。赤牌・5枚目の同牌は拒否します。
pub fn parse_tiles(input: &str) -> Result<Vec<Tile>, String> {
    let mut result = Vec::new();
    let mut digits = Vec::new();
    let mut multiplicities = [0_u8; 34];
    for character in input.chars().filter(|c| !c.is_whitespace()) {
        if ('1'..='9').contains(&character) {
            digits.push(character as u8 - b'0');
            if digits.len() > 136 {
                return Err("牌の入力が長すぎます".into());
            }
            continue;
        }
        let suit = match character {
            'm' | 'M' => Some((0, 9)),
            'p' | 'P' => Some((9, 9)),
            's' | 'S' => Some((18, 9)),
            'z' | 'Z' => Some((27, 7)),
            _ => None,
        };
        if let Some((offset, max_rank)) = suit {
            if digits.is_empty() {
                return Err(format!("{character} の前に牌の数字が必要です"));
            }
            for rank in digits.drain(..) {
                if rank > max_rank {
                    return Err(format!("字牌の数字は1〜7です: {rank}{character}"));
                }
                push_tile(&mut result, &mut multiplicities, offset + rank - 1)?;
            }
            continue;
        }
        if !digits.is_empty() {
            return Err("数字の後にはm・p・s・zが必要です".into());
        }
        let honor = match character {
            '東' => 27,
            '南' => 28,
            '西' => 29,
            '北' => 30,
            '白' => 31,
            '發' | '発' => 32,
            '中' => 33,
            ',' => continue,
            _ => return Err(format!("牌として読めない文字です: {character}")),
        };
        push_tile(&mut result, &mut multiplicities, honor)?;
    }
    if !digits.is_empty() {
        return Err("数字の後にはm・p・s・zが必要です".into());
    }
    Ok(result)
}

fn push_tile(output: &mut Vec<Tile>, multiplicities: &mut [u8; 34], id: u8) -> Result<(), String> {
    let count = &mut multiplicities[usize::from(id)];
    if *count == crate::rule_constants::COPIES_PER_TILE {
        return Err(format!("同じ牌は4枚までです: {}", Tile(id)));
    }
    *count += 1;
    output.push(Tile(id));
    Ok(())
}

/// 任意長の入力でも算術オーバーフローしません。
/// 返り値に5以上がある入力は、和了判定などの呼出先が不正として拒否します。
pub fn counts(tiles: &[Tile]) -> [u8; 34] {
    let mut result = [0_u8; 34];
    for tile in tiles {
        let slot = &mut result[usize::from(tile.id())];
        *slot = slot.saturating_add(1);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_is_valid_exactly_for_34_tiles() {
        for id in 0..=u8::MAX {
            assert_eq!(Tile::new(id).is_some(), id < 34);
        }
    }

    #[test]
    fn compact_text_round_trips_all_tiles() {
        for id in 0..34 {
            let tile = Tile::new(id).unwrap();
            assert_eq!(parse_tiles(&tile.short()).unwrap(), vec![tile]);
            assert!(!tile.japanese().is_empty());
        }
    }

    #[test]
    fn parser_rejects_red_incomplete_and_fifth_copies() {
        for input in ["0m", "8z", "123", "m", "11111p", "1東", "🙂"] {
            assert!(parse_tiles(input).is_err(), "{input}");
        }
        assert_eq!(parse_tiles("東 南 西 北 白 發 中").unwrap().len(), 7);
    }

    #[test]
    fn dora_wraps_each_suit_and_honor_family() {
        for (before, after) in [(8, 0), (17, 9), (26, 18), (30, 27), (33, 31)] {
            assert_eq!(Tile::new(before).unwrap().dora().id(), after);
        }
    }

    #[test]
    fn counting_arbitrary_input_does_not_overflow() {
        let many = vec![Tile::new(0).unwrap(); 1000];
        assert_eq!(counts(&many)[0], u8::MAX);
    }
}
