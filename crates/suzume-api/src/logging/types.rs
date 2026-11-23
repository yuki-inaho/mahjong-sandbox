use crate::config::{Algorithm, AppConfig, EffectiveAiConfig};
use crate::decision::DecisionView;
use crate::environment::EncodedObservation;
use crate::types::{ActionId, SeatId, Seed, TileId, Version, WireAction};
use crate::view::GameView;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use suzume::game::Game;

/// Completed is a core match end; Truncated is the configured operation limit;
/// Interrupted is an explicit earlier stop by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Completed,
    Truncated,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayHeader {
    pub version: Version,
    pub engine: String,
    pub rules_sha256: String,
    pub shuffle: String,
    pub seed: Seed,
    pub config: AppConfig,
    pub effective_ai: [Option<EffectiveAiConfig>; 4],
    pub initial_position_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayAction {
    pub sequence: u32,
    pub revision: u32,
    pub actor: SeatId,
    pub action: WireAction,
    pub before_sha256: String,
    pub after_sha256: String,
    pub decision: Option<DecisionView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayFooter {
    pub action_count: u32,
    pub revision: u32,
    pub reason: EndReason,
    pub scores: [i32; 4],
    pub riichi_pot: i32,
    pub final_position_sha256: String,
}

/// Deliberately has no seed field. Even an AI's private random seed is omitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicAiConfig {
    pub algorithm: Algorithm,
    pub max_evaluations: u32,
    pub lookahead_depth: u8,
    pub risk_weight: u16,
}
impl From<&EffectiveAiConfig> for PublicAiConfig {
    fn from(value: &EffectiveAiConfig) -> Self {
        Self {
            algorithm: value.algorithm,
            max_evaluations: value.max_evaluations,
            lookahead_depth: value.lookahead_depth,
            risk_weight: value.risk_weight,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicController {
    Human,
    Ai { config: PublicAiConfig },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicHeader {
    pub version: Version,
    pub engine: String,
    pub rules_sha256: String,
    pub shuffle: String,
    pub max_steps: u32,
    pub players: [PublicController; 4],
    pub initial: GameView,
}

/// Reaction choices remain private until the core resolves them. The public
/// view then shows only the selected meld or round result, not rejected choices.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicAction {
    Discard { tile: TileId },
    Riichi { tile: TileId },
    ClosedKan { tile: TileId },
    Tsumo,
    NextRound,
    ResponseSubmitted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicStep {
    pub sequence: u32,
    pub revision: u32,
    pub actor: SeatId,
    pub action: PublicAction,
    pub view: GameView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicFooter {
    pub action_count: u32,
    pub revision: u32,
    pub reason: EndReason,
    pub final_view: GameView,
}

/// One JSONL record. Full and public formats have disjoint headers and bodies.
/// Deserializing a record alone is not replay validation: use `replay` for that.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
pub enum LogRecord {
    ReplayHeader(ReplayHeader),
    ReplayAction(ReplayAction),
    ReplayFooter(ReplayFooter),
    PublicHeader(PublicHeader),
    PublicStep(PublicStep),
    PublicFooter(PublicFooter),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogLimits {
    /// Maximum UTF-8 JSON bytes before the newline, including a possible CR.
    pub max_line_bytes: usize,
    /// Includes the header and footer, not only action records.
    pub max_records: usize,
    /// Includes newline bytes and an optional final line without a newline.
    pub max_total_bytes: usize,
}
impl Default for LogLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: 1_048_576,
            max_records: 10_002,
            max_total_bytes: 268_435_456,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayResult {
    pub game: Game,
    pub metadata: ReplayHeader,
    pub termination: ReplayFooter,
}

/// One turn-based multi-agent transition. Session ID is fixed to 1 within a log.
/// `next_actor` names the next observer, which may differ from `actor`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatasetTransition {
    pub schema_version: Version,
    pub sequence: u32,
    pub actor: SeatId,
    pub observation: EncodedObservation,
    pub action: WireAction,
    pub action_id: ActionId,
    pub rewards: [i32; 4],
    pub escrow_delta: i32,
    pub next_actor: Option<SeatId>,
    pub next_observation: Option<EncodedObservation>,
    /// The last next-player observation at an external cutoff, for value
    /// bootstrapping. Natural termination and continuing steps keep this None.
    pub final_observation: Option<EncodedObservation>,
    pub terminated: bool,
    pub truncated: bool,
}
