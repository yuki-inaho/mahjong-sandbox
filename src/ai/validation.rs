use super::AiError;
use crate::game::{Action, Observation};
use crate::scoring::{Meld, MeldKind};
use crate::tile::{counts, Tile};

/// 形・範囲・公開枚数を検査します。役・振聴を含む最終合法性はGameが担います。
pub(super) fn validate(observation: &Observation, legal: &[Action]) -> Result<[u8; 34], AiError> {
    if legal.is_empty() {
        return Err(AiError::NoLegalActions);
    }
    if legal.len() > 189 {
        return Err(AiError::InvalidAction("合法手の一覧が上限を超えています"));
    }
    let invalid = AiError::InvalidObservation;
    if observation.seat >= 4 || observation.dealer >= 4 {
        return Err(invalid("観測の席が範囲外です"));
    }
    if observation.hand.len() > 14
        || observation.melds.len() > 4
        || observation.wall_remaining > 70
        || !(1..=5).contains(&observation.dora_indicators.len())
    {
        return Err(invalid("手牌・副露・山・表示牌の枚数が不正です"));
    }
    let own = &observation.players[observation.seat];
    if own.hand_len != observation.hand.len() || own.melds != observation.melds {
        return Err(invalid("自分の手牌・副露と公開情報が一致しません"));
    }
    let mut visible = [0_u16; 34];
    for tile in &observation.hand {
        visible[tile.id() as usize] += 1;
    }
    let mut full_hands = 0;
    for (seat, player) in observation.players.iter().enumerate() {
        if player.hand_len > 14 || player.melds.len() > 4 || player.discards.len() > 136 {
            return Err(invalid("公開された牌の枚数が不正です"));
        }
        let structural = player.hand_len + player.melds.len() * 3;
        if !(13..=14).contains(&structural) {
            return Err(invalid("公開手牌の構造枚数が不正です"));
        }
        full_hands += usize::from(structural == 14);
        if player.riichi && player.melds.iter().any(Meld::is_open) {
            return Err(invalid("リーチと開いた副露が矛盾しています"));
        }
        for meld in &player.melds {
            if !valid_meld(meld, seat) {
                return Err(invalid("副露の構造または取得元が不正です"));
            }
            for tile in &meld.tiles {
                visible[tile.id() as usize] += 1;
            }
        }
        // 鳴かれた捨牌はすでに副露として数えています。
        for discard in player.discards.iter().filter(|discard| !discard.called) {
            visible[discard.tile.id() as usize] += 1;
        }
    }
    if full_hands > 1 {
        return Err(invalid("構造上14枚の人が複数います"));
    }
    for tile in &observation.dora_indicators {
        visible[tile.id() as usize] += 1;
    }
    if visible.iter().any(|&amount| amount > 4) {
        return Err(invalid("公開情報と手牌だけで同じ牌が5枚以上あります"));
    }
    if let Some((seat, tile)) = observation.last_discard {
        if seat >= 4
            || observation.players[seat]
                .discards
                .last()
                .map(|discard| discard.tile != tile || discard.called)
                .unwrap_or(true)
        {
            return Err(invalid("応答する捨牌が公開河と一致しません"));
        }
    }
    let owned = counts(&observation.hand);
    for action in legal {
        validate_action(observation, action, &owned)?;
    }
    Ok(visible.map(|amount| amount as u8))
}

fn valid_meld(meld: &Meld, owner: usize) -> bool {
    let size = match meld.kind {
        MeldKind::Chi | MeldKind::Pon => 3,
        _ => 4,
    };
    if meld.tiles.len() != size
        || (meld.is_open() && !matches!(meld.from, Some(seat) if seat < 4 && seat != owner))
        || (!meld.is_open() && meld.from.is_some())
    {
        return false;
    }
    let mut tiles = meld.tiles.clone();
    tiles.sort_unstable();
    if meld.kind == MeldKind::Chi {
        is_sequence(&tiles)
    } else {
        tiles.iter().all(|tile| *tile == tiles[0])
    }
}

fn is_sequence(tiles: &[Tile]) -> bool {
    tiles.len() == 3
        && !tiles[0].is_honor()
        && tiles[0].rank() <= 7
        && tiles[1].id() == tiles[0].id() + 1
        && tiles[2].id() == tiles[0].id() + 2
}

fn validate_action(
    observation: &Observation,
    action: &Action,
    owned: &[u8; 34],
) -> Result<(), AiError> {
    let invalid = AiError::InvalidAction;
    let structural = observation.hand.len() + observation.melds.len() * 3;
    let player = &observation.players[observation.seat];
    match action {
        Action::Discard(index) | Action::Riichi(index) => {
            if *index >= observation.hand.len()
                || structural != 14
                || observation.last_discard.is_some()
            {
                return Err(invalid("打牌番号または打牌待ちの構造が不正です"));
            }
            if matches!(action, Action::Riichi(_))
                && (player.riichi
                    || observation.melds.iter().any(Meld::is_open)
                    || player.points < 1000
                    || observation.wall_remaining < 4)
            {
                return Err(invalid("リーチの基本条件を満たしません"));
            }
        }
        Action::Tsumo | Action::ClosedKan(_) => {
            if structural != 14 || observation.last_discard.is_some() {
                return Err(invalid("自分のツモ後の操作ではありません"));
            }
            if let Action::ClosedKan(tile) = action {
                if owned[tile.id() as usize] != 4
                    || !can_call(observation)
                    || observation.dora_indicators.len() == 5
                {
                    return Err(invalid("暗槓に必要な牌または条件がありません"));
                }
            }
        }
        Action::NextRound => {}
        _ => {
            let (from, tile) = observation
                .last_discard
                .ok_or(invalid("応答する捨牌がありません"))?;
            if from == observation.seat || structural != 13 {
                return Err(invalid("他家の捨牌へ応答する状態ではありません"));
            }
            match action {
                Action::Pon | Action::OpenKan => {
                    let needed = if *action == Action::Pon { 2 } else { 3 };
                    if !can_call(observation)
                        || owned[tile.id() as usize] < needed
                        || (*action == Action::OpenKan && observation.dora_indicators.len() == 5)
                    {
                        return Err(invalid("鳴きに必要な牌または条件がありません"));
                    }
                }
                Action::Chi(pair) => {
                    let mut group = vec![tile, pair[0], pair[1]];
                    group.sort_unstable();
                    if !can_call(observation)
                        || (from + 1) % 4 != observation.seat
                        || !is_sequence(&group)
                        || pair.iter().any(|tile| owned[tile.id() as usize] == 0)
                    {
                        return Err(invalid("チーの形または取得元が不正です"));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn can_call(observation: &Observation) -> bool {
    !observation.players[observation.seat].riichi
        && observation.wall_remaining > 0
        && observation.melds.len() < 4
}

/// RustのDefaultHasherやOS乱数に依存しない、固定した公開観測の識別値です。
pub(super) fn observation_seed(observation: &Observation, seed: u64) -> u64 {
    fn add(hash: &mut u64, value: u64) {
        for byte in value.to_le_bytes() {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for value in [
        seed,
        observation.seat as u64,
        observation.dealer as u64,
        observation.wall_remaining as u64,
    ] {
        add(&mut hash, value);
    }
    for tile in &observation.hand {
        add(&mut hash, u64::from(tile.id()));
    }
    for player in &observation.players {
        add(&mut hash, player.points as i64 as u64);
        add(&mut hash, player.hand_len as u64);
        add(&mut hash, u64::from(player.riichi));
        add(&mut hash, player.discards.len() as u64);
        for discard in &player.discards {
            add(
                &mut hash,
                u64::from(discard.tile.id())
                    | (u64::from(discard.called) << 8)
                    | (u64::from(discard.riichi) << 9),
            );
        }
        add(&mut hash, player.melds.len() as u64);
        for meld in &player.melds {
            add(
                &mut hash,
                match meld.kind {
                    MeldKind::Chi => 0,
                    MeldKind::Pon => 1,
                    MeldKind::OpenKan => 2,
                    MeldKind::ClosedKan => 3,
                },
            );
            add(
                &mut hash,
                meld.from.map(|seat| seat as u64 + 1).unwrap_or(0),
            );
            for tile in &meld.tiles {
                add(&mut hash, u64::from(tile.id()));
            }
        }
    }
    for tile in &observation.dora_indicators {
        add(&mut hash, u64::from(tile.id()));
    }
    if let Some((seat, tile)) = observation.last_discard {
        add(&mut hash, 1 + seat as u64);
        add(&mut hash, u64::from(tile.id()));
    }
    hash
}
