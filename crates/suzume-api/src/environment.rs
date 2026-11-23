//! Turn-based multi-agent environment. Rewards are raw score deltas in points.
//! Forking is a trusted experimentation capability, never part of Policy input.
use crate::{
    actions::{action_id, legal_mask, resolve_action},
    types::{ActionId, ApiError, ErrorCode, SeatId, Seed, Version, WireAction},
    view::GameView,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use suzume::{
    game::{Game, Phase},
    tile::counts,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EncodedObservation {
    pub schema_version: Version,
    pub actor: SeatId,
    pub view: GameView,
    #[schemars(length(min = 34, max = 34))]
    pub hand_counts: Vec<u8>,
    #[schemars(length(min = 34, max = 34))]
    pub visible_counts: Vec<u8>,
    #[schemars(length(min = 189, max = 189))]
    pub legal_mask: Vec<bool>,
}
impl EncodedObservation {
    pub fn for_actor(game: &Game, session_id: u32, revision: u32) -> Option<Self> {
        let actor = game.next_actor()?;
        let hand_counts = counts(&game.players()[actor].hand);
        let mut visible = hand_counts;
        for player in game.players() {
            for meld in &player.melds {
                for tile in &meld.tiles {
                    visible[usize::from(tile.id())] += 1;
                }
            }
            for discard in &player.discards {
                if !discard.called {
                    visible[usize::from(discard.tile.id())] += 1;
                }
            }
        }
        for tile in game.dora_indicators() {
            visible[usize::from(tile.id())] += 1;
        }
        let actor_id = SeatId::new(actor as u8).expect("core seat validated");
        Some(Self {
            schema_version: Version,
            actor: actor_id,
            view: GameView::from_game(game, Some(actor_id), session_id, revision),
            hand_counts: hand_counts.to_vec(),
            visible_counts: visible.to_vec(),
            legal_mask: legal_mask(game, actor),
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentTransition {
    pub session_id: u32,
    pub revision: u32,
    pub actor: Option<SeatId>,
    pub action: Option<WireAction>,
    pub action_id: Option<ActionId>,
    pub rewards: [i32; 4],
    pub escrow_delta: i32,
    pub terminated: bool,
    pub truncated: bool,
    pub observation: Option<EncodedObservation>,
    /// The final next-actor observation at a time limit, for value bootstrapping.
    /// Natural match ends and ongoing transitions have no final observation.
    pub final_observation: Option<EncodedObservation>,
    pub scores: [i32; 4],
    pub riichi_pot: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Environment {
    game: Game,
    session_id: u32,
    steps: u32,
    max_steps: u32,
}
impl Environment {
    pub fn new(seed: &Seed, max_steps: u32, session_id: u32) -> Result<Self, ApiError> {
        if !(1..=10_000).contains(&max_steps) {
            return Err(ApiError::new(
                ErrorCode::InvalidConfig,
                "max_steps must be 1..10000",
            ));
        }
        let game = Game::new(seed.value())
            .map_err(|e| ApiError::new(ErrorCode::InternalError, e.to_string()))?;
        Ok(Self {
            game,
            session_id,
            steps: 0,
            max_steps,
        })
    }
    pub fn session_id(&self) -> u32 {
        self.session_id
    }
    pub fn revision(&self) -> u32 {
        self.steps
    }
    pub fn terminated(&self) -> bool {
        matches!(self.game.phase(), Phase::MatchEnd)
    }
    pub fn truncated(&self) -> bool {
        !self.terminated() && self.steps >= self.max_steps
    }
    /// Trusted experiment state, including hidden information. Do not pass this to a player policy.
    pub fn game(&self) -> &Game {
        &self.game
    }
    pub fn fork(&self) -> Self {
        self.clone()
    }
    pub fn state(&self) -> EnvironmentTransition {
        self.transition(None, None, None, [0; 4], 0)
    }
    fn transition(
        &self,
        actor: Option<SeatId>,
        action: Option<WireAction>,
        id: Option<ActionId>,
        rewards: [i32; 4],
        escrow_delta: i32,
    ) -> EnvironmentTransition {
        let done = self.terminated() || self.truncated();
        EnvironmentTransition {
            session_id: self.session_id,
            revision: self.steps,
            actor,
            action,
            action_id: id,
            rewards,
            escrow_delta,
            terminated: self.terminated(),
            truncated: self.truncated(),
            observation: if done {
                None
            } else {
                EncodedObservation::for_actor(&self.game, self.session_id, self.steps)
            },
            final_observation: if self.truncated() {
                EncodedObservation::for_actor(&self.game, self.session_id, self.steps)
            } else {
                None
            },
            scores: self.game.scores(),
            riichi_pot: self.game.riichi_pot(),
        }
    }
    pub fn step(&mut self, id: ActionId) -> Result<EnvironmentTransition, ApiError> {
        if self.terminated() || self.truncated() {
            return Err(ApiError::new(
                ErrorCode::StepLimit,
                "environment has ended; reset before another step",
            ));
        }
        let actor = self
            .game
            .next_actor()
            .ok_or_else(|| ApiError::new(ErrorCode::WrongActor, "no next actor"))?;
        let action = resolve_action(&self.game, actor, id).ok_or_else(|| {
            ApiError::new(
                ErrorCode::IllegalAction,
                "action ID is not legal in this observation",
            )
        })?;
        let before = self.game.scores();
        let pot = self.game.riichi_pot();
        let wire = WireAction::from_core(&action).ok_or_else(|| {
            ApiError::new(
                ErrorCode::InternalError,
                "core action is outside the wire domain",
            )
        })?;
        debug_assert_eq!(action_id(&self.game, actor, &action), Some(id));
        self.game
            .apply(actor, action)
            .map_err(|e| ApiError::new(ErrorCode::IllegalAction, e.to_string()))?;
        self.steps += 1;
        Ok(self.transition(
            SeatId::new(actor as u8),
            Some(wire),
            Some(id),
            std::array::from_fn(|i| self.game.scores()[i] - before[i]),
            self.game.riichi_pot() - pot,
        ))
    }
}
