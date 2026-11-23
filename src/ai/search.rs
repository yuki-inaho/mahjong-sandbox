use super::validation::{observation_seed, validate};
use super::{AiAlgorithm, AiConfig, AiError, CandidateEvaluation, Decision, DecisionReason};
use crate::game::{Action, Observation};
use crate::hand::{shanten, winning_tiles};
use crate::scoring::{Meld, MeldKind};
use crate::tile::{counts, Tile};
use std::collections::BTreeMap;

type CacheKey = ([u8; 34], [u8; 34], usize);
const SHANTEN_WEIGHT: i64 = 2000;

struct Search {
    limit: u32,
    used: u32,
    exhausted: bool,
    cache: BTreeMap<CacheKey, i8>,
}
impl Search {
    fn new(limit: u32) -> Self {
        Self {
            limit,
            used: 0,
            exhausted: false,
            cache: BTreeMap::new(),
        }
    }
    fn distance(&mut self, hand: &[Tile], melds: &[Meld]) -> Option<i8> {
        let mut fixed = [0_u8; 34];
        for tile in melds.iter().flat_map(|meld| &meld.tiles) {
            fixed[tile.id() as usize] += 1;
        }
        let concealed = counts(hand);
        let key = (concealed, fixed, melds.len());
        if let Some(&value) = self.cache.get(&key) {
            return Some(value);
        }
        if self.used == self.limit {
            self.exhausted = true;
            return None;
        }
        self.used += 1;
        let mut value = shanten(hand, melds.len());
        // hand::shantenは副露の実体を受け取らないため、所有4枚の待ちをここで除きます。
        if value == 0
            && hand.len() % 3 == 1
            && !winning_tiles(hand, melds.len()).iter().any(|tile| {
                let id = tile.id() as usize;
                concealed[id] + fixed[id] < 4
            })
        {
            value = 1;
        }
        self.cache.insert(key, value);
        Some(value)
    }
    fn ukeire(&mut self, state: &Position, unseen: &[u8; 34]) -> Option<u16> {
        let distance = self.distance(&state.hand, &state.melds)?;
        let mut hand = state.hand.clone();
        let mut result = 0_u16;
        for (id, &remaining) in unseen.iter().enumerate() {
            if remaining == 0 {
                continue;
            }
            hand.push(Tile::new(id as u8)?);
            let next = self.distance(&hand, &state.melds)?;
            hand.pop();
            if next < distance {
                result += u16::from(remaining);
            }
        }
        Some(result)
    }
}

#[derive(Clone)]
struct Position {
    hand: Vec<Tile>,
    melds: Vec<Meld>,
    discarded: Option<Tile>,
}

pub(super) fn decide(
    config: AiConfig,
    observation: &Observation,
    legal: &[Action],
) -> Result<Decision, AiError> {
    config.validate()?;
    let visible = validate(observation, legal)?;
    let mut candidates: Vec<CandidateEvaluation> = legal
        .iter()
        .map(|action| CandidateEvaluation {
            action: action.clone(),
            shanten: None,
            ukeire: None,
            danger: match action {
                Action::Discard(index) | Action::Riichi(index) => {
                    danger(observation, observation.hand[*index])
                }
                _ => 0,
            },
            lookahead_value: None,
        })
        .collect();
    let immediate = legal
        .iter()
        .position(|action| matches!(action, Action::Ron | Action::Tsumo))
        .map(|index| (index, DecisionReason::Win))
        .or_else(|| {
            legal
                .iter()
                .position(|action| *action == Action::NextRound)
                .map(|index| (index, DecisionReason::Forced))
        })
        .or_else(|| (legal.len() == 1).then_some((0, DecisionReason::Forced)));
    if let Some((index, reason)) = immediate {
        return Ok(result(
            config,
            legal[index].clone(),
            reason,
            candidates,
            &Search::new(config.max_evaluations),
        ));
    }
    if config.algorithm == AiAlgorithm::Random {
        let mut rng = StableRandom(observation_seed(observation, config.seed));
        let index = rng.below(legal.len() as u64) as usize;
        return Ok(result(
            config,
            legal[index].clone(),
            DecisionReason::Random,
            candidates,
            &Search::new(0),
        ));
    }
    let mut search = Search::new(config.max_evaluations);
    let mut positions = Vec::with_capacity(legal.len());
    for (index, action) in legal.iter().enumerate() {
        let states = project(observation, action);
        let mut best: Option<(Position, (i8, u32, i32, u8))> = None;
        for state in states {
            let Some(distance) = search.distance(&state.hand, &state.melds) else {
                break;
            };
            let risk = state
                .discarded
                .map(|tile| danger(observation, tile))
                .unwrap_or(0);
            let key = (
                distance,
                u32::from(risk) * u32::from(config.risk_weight),
                -shape_value(&state.hand),
                state.discarded.map(Tile::id).unwrap_or(0),
            );
            if best
                .as_ref()
                .map(|(_, current)| key < *current)
                .unwrap_or(true)
            {
                best = Some((state, key));
            }
        }
        if let Some((state, key)) = best {
            candidates[index].shanten = Some(key.0);
            candidates[index].danger = state
                .discarded
                .map(|tile| danger(observation, tile))
                .unwrap_or(0);
            positions.push(Some(state));
        } else {
            positions.push(None);
        }
    }
    if config.algorithm == AiAlgorithm::Greedy {
        let selected = choose_greedy(
            observation,
            legal,
            &positions,
            &candidates,
            config.risk_weight,
        );
        let reason = if search.exhausted {
            DecisionReason::BudgetFallback
        } else {
            DecisionReason::Greedy
        };
        return Ok(result(
            config,
            legal[selected].clone(),
            reason,
            candidates,
            &search,
        ));
    }
    let unseen = visible.map(|amount| 4 - amount);
    for (index, state) in positions.iter().enumerate() {
        if let Some(state) = state {
            candidates[index].ukeire = search.ukeire(state, &unseen);
        }
    }
    let eligible = eligible(observation, legal, &candidates);
    // 有効牌数が途中までしか得られなかった候補を、0枚とみなして比較しません。
    if eligible
        .iter()
        .any(|&index| candidates[index].shanten.is_some() && candidates[index].ukeire.is_none())
    {
        let selected = choose_greedy(
            observation,
            legal,
            &positions,
            &candidates,
            config.risk_weight,
        );
        return Ok(result(
            config,
            legal[selected].clone(),
            DecisionReason::BudgetFallback,
            candidates,
            &search,
        ));
    }
    let mut ranked: Vec<usize> = eligible
        .into_iter()
        .filter(|&i| candidates[i].shanten.is_some())
        .collect();
    ranked.sort_by_key(|&i| {
        (
            std::cmp::Reverse(static_value(
                config,
                observation,
                &candidates[i],
                positions[i].as_ref().unwrap(),
            )),
            i,
        )
    });
    let fallback = choose_greedy(
        observation,
        legal,
        &positions,
        &candidates,
        config.risk_weight,
    );
    let mut selected = ranked.first().copied().unwrap_or(fallback);
    let mut reason = DecisionReason::Ukeire;
    if config.lookahead_depth == 2 {
        // 同じ手牌になる打牌/リーチや同牌の添字は一つの根として探索します。
        let mut roots = Vec::new();
        let mut root_hands = Vec::new();
        for &index in &ranked {
            if !matches!(legal[index], Action::Discard(_) | Action::Riichi(_))
                || candidates[index].ukeire.is_none()
            {
                continue;
            }
            let key = counts(&positions[index].as_ref().unwrap().hand);
            if root_hands.contains(&key) {
                continue;
            }
            root_hands.push(key);
            roots.push(index);
            if roots.len() == 3 {
                break;
            }
        }
        let mut all_completed = !roots.is_empty();
        for &index in &roots {
            let locked = matches!(legal[index], Action::Riichi(_))
                || observation.players[observation.seat].riichi;
            let value = lookahead(
                config,
                observation,
                positions[index].as_ref().unwrap(),
                &unseen,
                locked,
                &mut search,
            );
            candidates[index].lookahead_value = value;
            all_completed &= value.is_some();
        }
        // 未完了の根と完了した根を、異なる深さの数値で比較しません。
        if all_completed {
            selected = *roots
                .iter()
                .max_by_key(|&&i| {
                    (
                        candidates[i].lookahead_value.unwrap()
                            - i64::from(candidates[i].danger) * i64::from(config.risk_weight) * 20,
                        static_value(
                            config,
                            observation,
                            &candidates[i],
                            positions[i].as_ref().unwrap(),
                        ),
                        std::cmp::Reverse(i),
                    )
                })
                .unwrap();
            reason = DecisionReason::Lookahead;
        }
    }
    if search.exhausted {
        reason = DecisionReason::BudgetFallback;
    }
    Ok(result(
        config,
        legal[selected].clone(),
        reason,
        candidates,
        &search,
    ))
}

fn result(
    config: AiConfig,
    action: Action,
    reason: DecisionReason,
    candidates: Vec<CandidateEvaluation>,
    search: &Search,
) -> Decision {
    Decision {
        action,
        config,
        reason,
        candidates_evaluated: candidates
            .iter()
            .filter(|candidate| candidate.shanten.is_some())
            .count() as u32,
        candidates,
        evaluations: search.used,
        budget_exhausted: search.exhausted,
    }
}

fn project(observation: &Observation, action: &Action) -> Vec<Position> {
    let mut hand = observation.hand.clone();
    let mut melds = observation.melds.clone();
    match action {
        Action::Discard(index) | Action::Riichi(index) => {
            let discarded = hand.remove(*index);
            vec![Position {
                hand,
                melds,
                discarded: Some(discarded),
            }]
        }
        Action::Pass => vec![Position {
            hand,
            melds,
            discarded: None,
        }],
        Action::ClosedKan(tile) => {
            hand.retain(|candidate| candidate != tile);
            melds.push(Meld {
                kind: MeldKind::ClosedKan,
                tiles: vec![*tile; 4],
                from: None,
            });
            vec![Position {
                hand,
                melds,
                discarded: None,
            }]
        }
        Action::Pon | Action::OpenKan | Action::Chi(_) => {
            let Some((from, called)) = observation.last_discard else {
                return vec![];
            };
            let (kind, needed) = match action {
                Action::Pon => (MeldKind::Pon, vec![called; 2]),
                Action::OpenKan => (MeldKind::OpenKan, vec![called; 3]),
                Action::Chi(pair) => (MeldKind::Chi, pair.to_vec()),
                _ => return vec![],
            };
            let mut group = needed.clone();
            group.push(called);
            group.sort_unstable();
            for tile in needed {
                let Some(index) = hand.iter().position(|candidate| *candidate == tile) else {
                    return vec![];
                };
                hand.remove(index);
            }
            let mut forbidden = vec![called];
            if kind == MeldKind::Chi {
                let start = group[0].id();
                if called.id() == start && start % 9 <= 5 {
                    forbidden.push(Tile::new(start + 3).unwrap());
                }
                if called.id() == start + 2 && start % 9 >= 1 {
                    forbidden.push(Tile::new(start - 1).unwrap());
                }
            }
            melds.push(Meld {
                kind,
                tiles: group,
                from: Some(from),
            });
            if kind == MeldKind::OpenKan {
                return vec![Position {
                    hand,
                    melds,
                    discarded: None,
                }];
            }
            (0..hand.len())
                .filter(|&index| !forbidden.contains(&hand[index]))
                .map(|index| {
                    let mut after = hand.clone();
                    let discarded = after.remove(index);
                    Position {
                        hand: after,
                        melds: melds.clone(),
                        discarded: Some(discarded),
                    }
                })
                .collect()
        }
        _ => vec![],
    }
}

fn choose_greedy(
    observation: &Observation,
    legal: &[Action],
    positions: &[Option<Position>],
    candidates: &[CandidateEvaluation],
    risk_weight: u16,
) -> usize {
    let key = |index: usize| {
        let candidate = &candidates[index];
        (
            candidate.shanten.unwrap_or(8),
            u32::from(candidate.danger) * u32::from(risk_weight),
            positions[index]
                .as_ref()
                .map(|state| -shape_value(&state.hand))
                .unwrap_or(0),
            positions[index]
                .as_ref()
                .and_then(|state| state.discarded)
                .map(Tile::id)
                .unwrap_or(0),
        )
    };
    if let Some(index) = (0..legal.len())
        .filter(|&i| matches!(legal[i], Action::Riichi(_)) && candidates[i].shanten.is_some())
        .min_by_key(|&i| key(i))
    {
        return index;
    }
    let baseline = candidates
        .iter()
        .zip(legal)
        .filter(|(_, action)| matches!(action, Action::Discard(_) | Action::Pass))
        .filter_map(|(candidate, _)| candidate.shanten)
        .min()
        .unwrap_or(8);
    if let Some(index) = (0..legal.len()).find(|&i| {
        call_acceptable(observation, &legal[i])
            && candidates[i]
                .shanten
                .map(|distance| {
                    distance < baseline
                        || (matches!(legal[i], Action::ClosedKan(_) | Action::OpenKan)
                            && distance <= baseline)
                })
                .unwrap_or(false)
    }) {
        return index;
    }
    if let Some(index) = (0..legal.len())
        .filter(|&i| matches!(legal[i], Action::Discard(_)) && candidates[i].shanten.is_some())
        .min_by_key(|&i| key(i))
    {
        return index;
    }
    legal
        .iter()
        .position(|action| *action == Action::Pass)
        .unwrap_or(0)
}

fn eligible(
    observation: &Observation,
    legal: &[Action],
    candidates: &[CandidateEvaluation],
) -> Vec<usize> {
    let ready: Vec<usize> = (0..legal.len())
        .filter(|&i| {
            matches!(legal[i], Action::Riichi(_))
                && candidates[i]
                    .ukeire
                    .map(|amount| amount > 0)
                    .unwrap_or(false)
        })
        .collect();
    if !ready.is_empty() {
        return ready;
    }
    let baseline = candidates
        .iter()
        .zip(legal)
        .filter(|(_, action)| matches!(action, Action::Discard(_) | Action::Pass))
        .filter_map(|(candidate, _)| candidate.shanten)
        .min()
        .unwrap_or(8);
    (0..legal.len())
        .filter(|&i| match legal[i] {
            Action::Discard(_) | Action::Pass => true,
            Action::Riichi(_) => !legal
                .iter()
                .any(|action| matches!(action, Action::Discard(_))),
            _ => {
                call_acceptable(observation, &legal[i])
                    && candidates[i]
                        .shanten
                        .map(|distance| {
                            distance < baseline
                                || (matches!(legal[i], Action::OpenKan | Action::ClosedKan(_))
                                    && distance <= baseline)
                        })
                        .unwrap_or(false)
            }
        })
        .collect()
}

fn call_acceptable(observation: &Observation, action: &Action) -> bool {
    if matches!(action, Action::ClosedKan(_)) {
        return true;
    }
    let Some((_, called)) = observation.last_discard else {
        return false;
    };
    let needed = match action {
        Action::Pon => vec![called; 2],
        Action::OpenKan => vec![called; 3],
        Action::Chi(pair) => pair.to_vec(),
        _ => return false,
    };
    let value_honor = |tile: Tile| {
        tile.id() >= 31
            || tile.id() == 27
            || tile.id() == 27 + ((observation.seat + 4 - observation.dealer) % 4) as u8
    };
    let existing_honor = observation.melds.iter().any(|meld| {
        meld.tiles.iter().all(|tile| *tile == meld.tiles[0]) && value_honor(meld.tiles[0])
    });
    if existing_honor || (!matches!(action, Action::Chi(_)) && value_honor(called)) {
        return true;
    }
    !called.is_terminal_or_honor()
        && needed.iter().all(|tile| !tile.is_terminal_or_honor())
        && observation
            .hand
            .iter()
            .all(|tile| !tile.is_terminal_or_honor())
        && observation
            .melds
            .iter()
            .flat_map(|meld| &meld.tiles)
            .all(|tile| !tile.is_terminal_or_honor())
}

fn danger(observation: &Observation, tile: Tile) -> u16 {
    observation
        .players
        .iter()
        .enumerate()
        .filter(|(seat, player)| {
            *seat != observation.seat
                && player.riichi
                && !player.discards.iter().any(|discard| discard.tile == tile)
        })
        .map(|_| {
            if tile.is_honor() {
                1
            } else if tile.is_terminal() {
                2
            } else {
                3
            }
        })
        .sum()
}

fn shape_value(hand: &[Tile]) -> i32 {
    let values = counts(hand);
    let mut value = 0;
    for (id, &amount) in values.iter().enumerate() {
        if amount == 0 {
            continue;
        }
        value += match amount {
            2 => 6,
            3 => 12,
            4 => 9,
            _ => 0,
        };
        if id < 27 {
            if id % 9 <= 7 && values[id + 1] > 0 {
                value += 4;
            }
            if id % 9 <= 6 && values[id + 2] > 0 {
                value += 2;
            }
        }
    }
    value
}

fn dora_value(observation: &Observation, position: &Position) -> i64 {
    let mut count = 0;
    for indicator in &observation.dora_indicators {
        let dora = indicator.dora();
        count += position
            .hand
            .iter()
            .chain(position.melds.iter().flat_map(|meld| &meld.tiles))
            .filter(|&&tile| tile == dora)
            .count() as i64;
    }
    count * 15
}

fn static_value(
    config: AiConfig,
    observation: &Observation,
    candidate: &CandidateEvaluation,
    position: &Position,
) -> i64 {
    -i64::from(candidate.shanten.unwrap_or(8)) * SHANTEN_WEIGHT
        + i64::from(candidate.ukeire.unwrap_or(0)) * 10
        + i64::from(shape_value(&position.hand))
        + dora_value(observation, position)
        - i64::from(candidate.danger) * i64::from(config.risk_weight) * 20
}

/// 真の山を使わず、公開情報と矛盾しない牌から4回標本を取ります。
/// 各標本で次の打牌を最大2候補に絞り、その先の有効牌を全種類検査します。
fn lookahead(
    config: AiConfig,
    observation: &Observation,
    position: &Position,
    unseen: &[u8; 34],
    locked: bool,
    search: &mut Search,
) -> Option<i64> {
    if observation.wall_remaining == 0 {
        return None;
    }
    let total: u64 = unseen.iter().map(|&n| u64::from(n)).sum();
    if total == 0 {
        return None;
    }
    let mut rng = StableRandom(observation_seed(observation, config.seed));
    let mut value = 0_i64;
    for _ in 0..4 {
        let mut offset = rng.below(total);
        let mut drawn = None;
        for (id, &amount) in unseen.iter().enumerate() {
            if offset < u64::from(amount) {
                drawn = Tile::new(id as u8);
                break;
            }
            offset -= u64::from(amount);
        }
        let drawn = drawn?;
        let mut hand = position.hand.clone();
        hand.push(drawn);
        let distance = search.distance(&hand, &position.melds)?;
        if distance < 0 {
            value += SHANTEN_WEIGHT + dora_value(observation, position);
            continue;
        }
        let mut children = Vec::new();
        let mut used_tiles = Vec::new();
        for index in 0..hand.len() {
            if (locked && index != hand.len() - 1) || used_tiles.contains(&hand[index]) {
                continue;
            }
            used_tiles.push(hand[index]);
            let mut after = hand.clone();
            let discard = after.remove(index);
            let child = Position {
                hand: after,
                melds: position.melds.clone(),
                discarded: Some(discard),
            };
            let next = search.distance(&child.hand, &child.melds)?;
            let risk = danger(observation, discard);
            let key = (
                next,
                u32::from(risk) * u32::from(config.risk_weight),
                -shape_value(&child.hand),
                discard.id(),
            );
            children.push((key, child));
        }
        children.sort_by_key(|(key, _)| *key);
        let mut remaining = *unseen;
        remaining[drawn.id() as usize] -= 1;
        let mut best = None;
        for (key, child) in children.into_iter().take(2) {
            let outs = search.ukeire(&child, &remaining)?;
            let candidate = CandidateEvaluation {
                action: Action::Pass,
                shanten: Some(key.0),
                ukeire: Some(outs),
                danger: child
                    .discarded
                    .map(|tile| danger(observation, tile))
                    .unwrap_or(0),
                lookahead_value: None,
            };
            let score = static_value(config, observation, &candidate, &child);
            best = Some(best.map(|current: i64| current.max(score)).unwrap_or(score));
        }
        value += best?;
    }
    Some(value / 4)
}

struct StableRandom(u64);
impl StableRandom {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    fn below(&mut self, upper: u64) -> u64 {
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next();
            if value >= threshold {
                return value % upper;
            }
        }
    }
}
