//! Interoperable four-player mjai events and bounded `.mjson` inspection.
//!
//! This module reconstructs what the record says. It does not replay the Suzume
//! game engine, prove scoring or legality, infer unknown tiles, or supply a legal
//! action mask for an external rule set. Full events are trusted offline data;
//! use [`ReplayState::observe`] to construct a player's information boundary.
//!
//! The input is decompressed UTF-8 NDJSON. Every callback result is provisional
//! until [`visit`] returns successfully after `end_game` and clean EOF.

mod reader;
mod state;
mod types;
mod writer;

pub use reader::{analyze, parse_event, visit};
pub use state::ReplayState;
pub use types::*;
pub use writer::MjaiWriter;

use crate::types::{ApiError, ErrorCode};

fn error(line: usize, kind: &str, message: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        ErrorCode::LogError,
        format!("mjson line {line} ({kind}): {message}"),
    )
}
