//! Versioned, bounded JSON Lines for trusted replay and public sharing.
//!
//! Full logs reveal every hidden tile through their seed. Public logs cannot be
//! replayed and never contain private indices, AI decisions, or hidden hashes.
//! Replay hashes cover a bounded view projection, not the complete internal Game.
//! Stream exporters may have called the consumer before a later input error;
//! publish an output file only after the complete input has been accepted.

mod reader;
mod types;
mod writer;

pub use reader::{export_transitions, replay};
pub use types::*;
pub use writer::LogWriter;

use crate::config::EffectiveAiConfig;
use crate::decision::DecisionView;
use crate::types::{ApiError, ErrorCode, SeatId, WireAction};
use crate::view::GameView;
use sha2::{Digest, Sha256};
use suzume::game::{Game, Phase};

pub const SHUFFLE_ALGORITHM: &str = "splitmix64-fisher-yates-v1";
const LOG_SESSION_ID: u32 = 1;

fn error(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::LogError, message)
}

fn seat(value: usize) -> Result<SeatId, ApiError> {
    u8::try_from(value)
        .ok()
        .and_then(SeatId::new)
        .ok_or_else(|| error("actor must be a seat in 0..3"))
}

/// Deterministic struct-field JSON ordering; no maps or history-dependent text.
/// Includes each seat's private view, but not wall order, latent core flags,
/// random generator state, cumulative events, or cumulative action history.
fn position_sha256(game: &Game, revision: u32) -> Result<String, ApiError> {
    let views: [GameView; 4] = std::array::from_fn(|index| {
        GameView::from_game(
            game,
            Some(SeatId::new(index as u8).expect("fixed seat")),
            LOG_SESSION_ID,
            revision,
        )
    });
    let bytes = serde_json::to_vec(&views).map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_reason(
    game: &Game,
    sequence: u32,
    max_steps: u32,
    reason: EndReason,
) -> Result<(), ApiError> {
    let terminated = matches!(game.phase(), Phase::MatchEnd);
    let valid = match reason {
        EndReason::Completed => terminated,
        EndReason::Truncated => !terminated && sequence == max_steps,
        EndReason::Interrupted => !terminated && sequence < max_steps,
    };
    if !valid {
        return Err(error(
            "footer reason does not match the match state and operation limit",
        ));
    }
    Ok(())
}

fn validate_decision(
    decision: Option<&DecisionView>,
    action: &WireAction,
    effective: Option<&EffectiveAiConfig>,
) -> Result<(), ApiError> {
    if let Some(decision) = decision {
        if &decision.action != action || Some(&decision.config) != effective {
            return Err(error(
                "decision action or effective AI configuration does not match the log",
            ));
        }
        if decision.evaluations > decision.config.max_evaluations
            || decision.candidates.len() > crate::actions::ACTION_SPACE
            || decision.candidates_evaluated as usize > decision.candidates.len()
        {
            return Err(error("decision counters exceed their declared bounds"));
        }
    }
    Ok(())
}
