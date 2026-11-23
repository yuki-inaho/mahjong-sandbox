//! 公開情報だけを受け取るCPU。強さの保証や学習済みモデルへの依存はありません。

use crate::game::{Action, Observation};
use crate::hand::shanten;
use crate::scoring::Meld;
use crate::tile::{counts, Tile};

/// 判断方式の差替え点。対局の状態を直接変更する権限を持ちません。
pub trait Policy {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GreedyAi;

impl Policy for GreedyAi {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action> {
        if let Some(action) = legal
            .iter()
            .find(|a| matches!(a, Action::Ron | Action::Tsumo | Action::NextRound))
        {
            return Some(action.clone());
        }
        let ready: Vec<&Action> = legal
            .iter()
            .filter(|a| matches!(a, Action::Riichi(_)))
            .collect();
        if let Some(action) = ready.into_iter().min_by_key(|action| match action {
            Action::Riichi(index) => discard_cost(observation, *index),
            _ => unreachable!(),
        }) {
            return Some(action.clone());
        }
        if let Some(action) = choose_call(observation, legal) {
            return Some(action);
        }
        let discarded = legal
            .iter()
            .filter_map(|action| match action {
                Action::Discard(index) => Some((*index, action)),
                _ => None,
            })
            .min_by_key(|&(index, _)| discard_cost(observation, index));
        if let Some((_, action)) = discarded {
            return Some(action.clone());
        }
        legal
            .iter()
            .find(|a| **a == Action::Pass)
            .or_else(|| legal.first())
            .cloned()
    }
}

/// 第一の基準は向聴数。公開河の安全度と残る牌のつながりを同順位の比較に使います。
fn discard_cost(observation: &Observation, index: usize) -> (i8, i32, i32, u8) {
    let mut hand = observation.hand.clone();
    let discarded = hand.remove(index);
    let distance = shanten(&hand, observation.melds.len());
    let mut danger = 0;
    for (seat, player) in observation.players.iter().enumerate() {
        if seat != observation.seat
            && player.riichi
            && !player.discards.iter().any(|d| d.tile == discarded)
        {
            danger += if discarded.is_honor() {
                1
            } else if discarded.is_terminal() {
                2
            } else {
                3
            };
        }
    }
    (distance, danger, -shape_value(&hand), discarded.id())
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

fn choose_call(observation: &Observation, legal: &[Action]) -> Option<Action> {
    let baseline = if observation.hand.len() % 3 == 2 {
        (0..observation.hand.len())
            .map(|i| discard_cost(observation, i).0)
            .min()
            .unwrap_or(8)
    } else {
        shanten(&observation.hand, observation.melds.len())
    };
    for action in legal {
        let (needed, called, kan) = match action {
            Action::ClosedKan(tile) => (vec![*tile; 4], *tile, true),
            Action::OpenKan => {
                let tile = observation.last_discard?.1;
                (vec![tile; 3], tile, true)
            }
            Action::Pon => {
                let tile = observation.last_discard?.1;
                (vec![tile; 2], tile, false)
            }
            Action::Chi(pair) => (pair.to_vec(), observation.last_discard?.1, false),
            _ => continue,
        };
        let called_group_is_simple = !called.is_terminal_or_honor()
            && needed.iter().all(|tile| !tile.is_terminal_or_honor());
        let mut remaining = observation.hand.clone();
        for required in needed {
            let index = remaining.iter().position(|&t| t == required)?;
            remaining.remove(index);
        }
        let closed = matches!(action, Action::ClosedKan(_));
        let value_honor = is_value_honor(called, observation.seat, observation.dealer)
            && !matches!(action, Action::Chi(_));
        let existing_honor = observation.melds.iter().any(|meld| {
            meld.tiles.iter().all(|t| *t == meld.tiles[0])
                && is_value_honor(meld.tiles[0], observation.seat, observation.dealer)
        });
        let all_simples = called_group_is_simple
            && remaining.iter().all(|t| !t.is_terminal_or_honor())
            && melds_are_simple(&observation.melds);
        if !closed && !value_honor && !existing_honor && !all_simples {
            continue;
        }
        let distance = if kan {
            shanten(&remaining, observation.melds.len() + 1)
        } else {
            (0..remaining.len())
                .filter_map(|i| {
                    if remaining[i] == called {
                        return None;
                    }
                    let mut after = remaining.clone();
                    after.remove(i);
                    Some(shanten(&after, observation.melds.len() + 1))
                })
                .min()
                .unwrap_or(8)
        };
        if distance < baseline || (kan && distance <= baseline) {
            return Some(action.clone());
        }
    }
    None
}

fn is_value_honor(tile: Tile, seat: usize, dealer: usize) -> bool {
    tile.id() >= 31 || tile.id() == 27 || tile.id() == 27 + ((seat + 4 - dealer) % 4) as u8
}

fn melds_are_simple(melds: &[Meld]) -> bool {
    melds
        .iter()
        .all(|m| m.tiles.iter().all(|t| !t.is_terminal_or_honor()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

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
        let view = game.observation(0).unwrap();
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
        view.hand = crate::tile::parse_tiles("13m234456p34556s").unwrap();
        view.melds.clear();
        view.last_discard = Some((3, Tile::new(1).unwrap()));
        let chi = Action::Chi([Tile::new(0).unwrap(), Tile::new(2).unwrap()]);
        assert_eq!(
            GreedyAi.choose_action(&view, &[Action::Pass, chi]),
            Some(Action::Pass)
        );
    }
}
