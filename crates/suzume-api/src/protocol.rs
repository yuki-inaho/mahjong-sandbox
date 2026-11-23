//! The Rust source of truth for JSON, TOML-related values and generated TypeScript.
use crate::{
    config::{AiConfigSpec, AppConfig, EffectiveAiConfig},
    decision::DecisionView,
    environment::EnvironmentTransition,
    types::{ActionId, ErrorCode, Seed, Version, WireAction},
    view::GameView,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
fn default_limit() -> u32 {
    10_000
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    New {
        version: Version,
        #[serde(default)]
        config: AppConfig,
    },
    LoadConfig {
        version: Version,
        toml: String,
    },
    View {
        version: Version,
    },
    Apply {
        version: Version,
        session_id: u32,
        expected_revision: u32,
        action: WireAction,
    },
    Step {
        version: Version,
        session_id: u32,
        expected_revision: u32,
    },
    Analyze {
        version: Version,
        session_id: u32,
        expected_revision: u32,
        config: AiConfigSpec,
    },
    InspectConfig {
        version: Version,
        toml: String,
    },
    EnvironmentReset {
        version: Version,
        seed: Seed,
        #[serde(default = "default_limit")]
        max_steps: u32,
    },
    EnvironmentView {
        version: Version,
    },
    EnvironmentStep {
        version: Version,
        session_id: u32,
        expected_revision: u32,
        action_id: ActionId,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResponseData {
    Game {
        state: Box<GameView>,
    },
    Analysis {
        session_id: u32,
        revision: u32,
        decision: DecisionView,
    },
    Config {
        config: Box<AppConfig>,
        effective_ai: [Option<EffectiveAiConfig>; 4],
    },
    Environment {
        transition: Box<EnvironmentTransition>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Ok {
        version: Version,
        data: Box<ResponseData>,
    },
    Error {
        version: Version,
        code: ErrorCode,
        message: String,
        session_id: Option<u32>,
        revision: Option<u32>,
    },
}
/// Schema root keeps every independently consumable value in definitions.
#[derive(JsonSchema)]
pub struct ProtocolSchema {
    pub request: Request,
    pub response: Response,
    pub config: AppConfig,
    pub log_record: crate::logging::LogRecord,
    pub dataset_transition: crate::logging::DatasetTransition,
}
