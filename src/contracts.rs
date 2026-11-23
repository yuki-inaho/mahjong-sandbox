//! C-01〜C-06: ゲームの実行経路と形式検証で共有する、状態を壊さない基本操作。
//!
//! 公開関数は不正入力を拒否します。点数は全検査の完了後にだけ書き換えます。
//! 契約の数式は `rules/contracts.json` を正本とし、Kaniで実装との対応を検査します。
//!
//! 比較用の生成仕様は公開APIではありません。
//! ```compile_fail
//! use suzume::contracts::specification;
//! ```

use crate::rule_constants::{
    COPIES_PER_TILE, PLAYER_COUNT, RIICHI_MIN_WALL, RIICHI_STICK, TILE_KINDS,
};
use std::fmt;

/// 宣言式から生成した比較用の仕様。ゲーム側での変更は禁止します。
// 通常ビルドでは宣言照合用の定数だけを使い、関係式の呼出しはKaniが担当します。
#[cfg_attr(not(kani), allow(dead_code))]
#[path = "generated_contracts.rs"]
pub(crate) mod specification;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidAccount,
    SameAccount,
    NegativeAmount,
    ArithmeticOverflow,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidAccount => "点数の移動先または移動元が不正です",
            Self::SameAccount => "同じ口座間で点数は移動できません",
            Self::NegativeAmount => "移動する点数は0以上である必要があります",
            Self::ArithmeticOverflow => "点数が整数の範囲を超えます",
        })
    }
}

impl std::error::Error for ContractError {}

/// C-01: 有効な席から次の席へ進みます。不正な席には値を返しません。
pub fn next_seat(seat: u8) -> Option<u8> {
    if usize::from(seat) >= PLAYER_COUNT {
        None
    } else if usize::from(seat) + 1 == PLAYER_COUNT {
        Some(0)
    } else {
        Some(seat + 1)
    }
}

/// C-02: 表示牌からドラを求めます。数牌・風牌・三元牌は別々に循環します。
pub fn dora_from_indicator(id: u8) -> Option<u8> {
    if usize::from(id) >= TILE_KINDS {
        return None;
    }
    Some(match id {
        8 | 17 | 26 => id - 8,
        30 => 27,
        33 => 31,
        _ => id + 1,
    })
}

/// C-03: 同種牌の二つの保管場所間で、1枚だけ移動する値を返します。
pub fn move_one(from: u8, to: u8) -> Option<(u8, u8)> {
    if from == 0 || from > COPIES_PER_TILE || to >= COPIES_PER_TILE {
        None
    } else {
        Some((from - 1, to + 1))
    }
}

/// C-04: 四人の持点と供託（添字4）の間で点数を原子的に移動します。
pub fn transfer_points(
    accounts: &mut [i32; 5],
    from: usize,
    to: usize,
    amount: i32,
) -> Result<(), ContractError> {
    if from >= accounts.len() || to >= accounts.len() {
        return Err(ContractError::InvalidAccount);
    }
    if from == to {
        return Err(ContractError::SameAccount);
    }
    if amount < 0 {
        return Err(ContractError::NegativeAmount);
    }
    // 両側の検査を終えてから書き換えるため、途中失敗で片側だけ動きません。
    let new_source = accounts[from]
        .checked_sub(amount)
        .ok_or(ContractError::ArithmeticOverflow)?;
    let new_destination = accounts[to]
        .checked_add(amount)
        .ok_or(ContractError::ArithmeticOverflow)?;
    accounts[from] = new_source;
    accounts[to] = new_destination;
    Ok(())
}

/// C-05: リーチ宣言に必要な点数・門前・聴牌・残り牌・未宣言を検査します。
pub fn can_declare_riichi(
    closed: bool,
    tenpai: bool,
    points: i32,
    wall: usize,
    already: bool,
) -> bool {
    closed && tenpai && points >= RIICHI_STICK && wall >= RIICHI_MIN_WALL && !already
}

/// C-06: リーチ後はツモ牌だけを打牌できます。牌IDの不正も拒否します。
pub fn discard_allowed(riichi: bool, drawn: u8, discard: u8) -> bool {
    usize::from(drawn) < TILE_KINDS
        && usize::from(discard) < TILE_KINDS
        && (!riichi || drawn == discard)
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// u8の全入力。成功値が宣言関係を満たし、無効IDは拒否されます。
    #[kani::proof]
    fn next_seat_refines_declaration() {
        let seat: u8 = kani::any();
        let result = next_seat(seat);
        assert_eq!(result.is_some(), usize::from(seat) < PLAYER_COUNT);
        if let Some(next) = result {
            assert!(specification::next_seat_relation(
                u16::from(seat),
                u16::from(next)
            ));
            assert!(usize::from(next) < PLAYER_COUNT);
        }
    }

    #[kani::proof]
    fn dora_refines_declaration() {
        let tile: u8 = kani::any();
        let result = dora_from_indicator(tile);
        assert_eq!(result.is_some(), usize::from(tile) < TILE_KINDS);
        if let Some(dora) = result {
            assert!(specification::dora_relation(
                u16::from(tile),
                u16::from(dora)
            ));
            assert!(usize::from(dora) < TILE_KINDS);
        }
    }

    #[kani::proof]
    fn move_one_refines_declaration_and_conserves() {
        let from: u8 = kani::any();
        let to: u8 = kani::any();
        let result = move_one(from, to);
        assert_eq!(
            result.is_some(),
            from > 0 && from <= COPIES_PER_TILE && to < COPIES_PER_TILE
        );
        if let Some((new_from, new_to)) = result {
            assert!(specification::move_one_relation(
                u16::from(from),
                u16::from(to),
                u16::from(new_from),
                u16::from(new_to)
            ));
            assert!(new_from <= COPIES_PER_TILE && new_to <= COPIES_PER_TILE);
        }
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn point_transfer_refines_declaration_and_is_atomic() {
        let before: [i32; 5] = kani::any();
        let from: usize = kani::any();
        let to: usize = kani::any();
        let amount: i32 = kani::any();
        let mut after = before;
        let legal = from < 5
            && to < 5
            && specification::point_transfer_legal(
                from as u64,
                to as u64,
                i64::from(before[from]),
                i64::from(before[to]),
                i64::from(amount),
            );
        let result = transfer_points(&mut after, from, to, amount);
        assert_eq!(result.is_ok(), legal);
        if result.is_ok() {
            assert!(specification::point_transfer_relation(
                i64::from(before[from]),
                i64::from(before[to]),
                i64::from(amount),
                i64::from(after[from]),
                i64::from(after[to])
            ));
            assert_eq!(ledger_total(&before), ledger_total(&after));
            for index in 0..5 {
                if index != from && index != to {
                    assert_eq!(after[index], before[index]);
                }
            }
        } else {
            // 配列全体の等値比較は内部memcmpの20バイト走査を展開します。
            // 同じ性質を5口座それぞれについて明示し、検証境界を要素数に一致させます。
            for index in 0..5 {
                assert_eq!(after[index], before[index]);
            }
        }
    }

    #[kani::proof]
    fn riichi_refines_declaration() {
        let closed: bool = kani::any();
        let tenpai: bool = kani::any();
        let points: i32 = kani::any();
        let wall: usize = kani::any();
        let already: bool = kani::any();
        assert_eq!(
            can_declare_riichi(closed, tenpai, points, wall, already),
            specification::riichi_legal(closed, tenpai, i64::from(points), wall as u64, already)
        );
    }

    #[kani::proof]
    fn discard_refines_declaration() {
        let riichi: bool = kani::any();
        let drawn: u8 = kani::any();
        let discard: u8 = kani::any();
        assert_eq!(
            discard_allowed(riichi, drawn, discard),
            specification::discard_legal(riichi, u16::from(drawn), u16::from(discard))
        );
    }
}

/// 供託を含めた総点。i32を5個加算してもi64の範囲には収まります。
pub fn ledger_total(accounts: &[i32; 5]) -> i64 {
    accounts.iter().map(|&points| i64::from(points)).sum()
}

/// 現在ビルドに含まれる宣言と生成物が同一かを検査します。
pub fn declarations_current() -> bool {
    crate::rule_constants::RULE_SOURCE == include_str!("../rules/riichi.rules")
        && specification::CONTRACT_SOURCE == include_str!("../rules/contracts.json")
        && crate::rule_constants::DECLARATION_SHA256 == specification::DECLARATION_SHA256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_seat_wraps_and_rejects_invalid() {
        assert_eq!(next_seat(0), Some(1));
        assert_eq!(next_seat(3), Some(0));
        assert_eq!(next_seat(4), None);
        assert_eq!(next_seat(255), None);
    }

    #[test]
    fn dora_wraps_inside_each_family() {
        for (indicator, dora) in [(0, 1), (8, 0), (17, 9), (26, 18), (30, 27), (33, 31)] {
            assert_eq!(dora_from_indicator(indicator), Some(dora));
        }
        assert_eq!(dora_from_indicator(34), None);
    }

    #[test]
    fn tile_move_preserves_count_and_rejects_empty_or_full() {
        assert_eq!(move_one(4, 0), Some((3, 1)));
        assert_eq!(move_one(1, 3), Some((0, 4)));
        assert_eq!(move_one(0, 0), None);
        assert_eq!(move_one(2, 4), None);
        assert_eq!(move_one(5, 0), None);
    }

    #[test]
    fn points_transfer_preserves_total_including_escrow() {
        let mut accounts = [25000, 25000, 25000, 25000, 0];
        transfer_points(&mut accounts, 0, 4, 1000).unwrap();
        assert_eq!(accounts, [24000, 25000, 25000, 25000, 1000]);
        transfer_points(&mut accounts, 4, 2, 1000).unwrap();
        assert_eq!(ledger_total(&accounts), 100000);
    }

    #[test]
    fn rejected_transfer_leaves_every_account_unchanged() {
        let initial = [i32::MIN, i32::MAX, 25000, 25000, 0];
        for (from, to, amount) in [(0, 1, 1), (2, 1, 1), (8, 1, 1), (1, 1, 1), (2, 3, -1)] {
            let mut accounts = initial;
            assert!(transfer_points(&mut accounts, from, to, amount).is_err());
            assert_eq!(accounts, initial);
        }
    }

    #[test]
    fn riichi_requires_every_declared_precondition() {
        assert!(can_declare_riichi(true, true, 1000, 4, false));
        assert!(!can_declare_riichi(false, true, 1000, 4, false));
        assert!(!can_declare_riichi(true, false, 1000, 4, false));
        assert!(!can_declare_riichi(true, true, 999, 4, false));
        assert!(!can_declare_riichi(true, true, 1000, 3, false));
        assert!(!can_declare_riichi(true, true, 1000, 4, true));
    }

    #[test]
    fn riichi_discard_is_drawn_tile_and_ids_are_valid() {
        assert!(discard_allowed(false, 1, 2));
        assert!(discard_allowed(true, 33, 33));
        assert!(!discard_allowed(true, 1, 2));
        assert!(!discard_allowed(false, 34, 2));
        assert!(!discard_allowed(false, 1, 34));
    }

    #[test]
    fn checked_in_declarations_match_generated_outputs() {
        assert!(declarations_current());
    }
}
