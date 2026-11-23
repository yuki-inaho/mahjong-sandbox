//! Stable, tile-based actions for learning and search. Reserved IDs are never legal.
use crate::types::{ActionId, HandIndex, WireAction};
use suzume::game::{Action, Game, Phase};
pub const ACTION_SPACE: usize = 189;

pub fn action_id(game: &Game, actor: usize, action: &Action) -> Option<ActionId> {
    let hand = &game.players().get(actor)?.hand;
    let id = match action {
        Action::Discard(i) => u16::from(hand.get(*i)?.id()),
        Action::Riichi(i) => 34 + u16::from(hand.get(*i)?.id()),
        Action::Tsumo => 68,
        Action::Ron => 69,
        Action::Pon => 70,
        Action::OpenKan => 71,
        Action::Pass => 72,
        Action::NextRound => 73,
        Action::ClosedKan(tile) => 74 + u16::from(tile.id()),
        Action::Chi(pair) => {
            let called = match game.phase() {
                Phase::Reaction { tile, .. } => *tile,
                _ => return None,
            };
            let start = called.id().min(pair[0].id()).min(pair[1].id());
            if start >= 27 || start % 9 > 6 {
                return None;
            }
            let mut group = [called.id(), pair[0].id(), pair[1].id()];
            group.sort_unstable();
            if group != [start, start + 1, start + 2] {
                return None;
            }
            108 + u16::from(start) * 3 + u16::from(called.id() - start)
        }
    };
    ActionId::new(id)
}

pub fn legal_mask(game: &Game, actor: usize) -> Vec<bool> {
    let mut mask = vec![false; ACTION_SPACE];
    if game.next_actor() != Some(actor) {
        return mask;
    }
    for action in game.legal_actions(actor) {
        if let Some(id) = action_id(game, actor, &action) {
            mask[usize::from(id.get())] = true;
        }
    }
    mask
}

/// Equivalent copies share an ID; the first matching legal core action is canonical.
pub fn resolve_action(game: &Game, actor: usize, id: ActionId) -> Option<Action> {
    if game.next_actor() != Some(actor) {
        return None;
    }
    game.legal_actions(actor)
        .into_iter()
        .find(|action| action_id(game, actor, action) == Some(id))
}

/// Useful when documenting that wire actions retain exact physical hand positions.
pub fn discard_at(index: u8) -> Option<WireAction> {
    Some(WireAction::Discard {
        index: HandIndex::new(index)?,
    })
}
