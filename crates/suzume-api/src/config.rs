//! One configuration model for TOML, JSON, applications and replay metadata.
use crate::types::{ApiError, ErrorCode, SeatId, Seed, Version};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use suzume::ai::{AiAlgorithm, AiConfig, AiPreset};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Random,
    Greedy,
    Ukeire,
}
impl Algorithm {
    fn core(self) -> AiAlgorithm {
        match self {
            Self::Random => AiAlgorithm::Random,
            Self::Greedy => AiAlgorithm::Greedy,
            Self::Ukeire => AiAlgorithm::Ukeire,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Beginner,
    Standard,
    Strong,
}
impl Preset {
    fn core(self) -> AiPreset {
        match self {
            Self::Beginner => AiPreset::Beginner,
            Self::Standard => AiPreset::Standard,
            Self::Strong => AiPreset::Strong,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AiConfigSpec {
    #[serde(default)]
    pub preset: Option<Preset>,
    #[serde(default)]
    pub algorithm: Option<Algorithm>,
    #[serde(default)]
    pub seed: Seed,
    #[serde(default)]
    pub max_evaluations: Option<u32>,
    #[serde(default)]
    pub lookahead_depth: Option<u8>,
    #[serde(default)]
    pub risk_weight: Option<u16>,
}
impl AiConfigSpec {
    pub fn resolve(&self) -> Result<AiConfig, ApiError> {
        if self.preset.is_some() && self.algorithm.is_some() {
            return Err(ApiError::new(
                ErrorCode::InvalidConfig,
                "preset and algorithm are mutually exclusive",
            ));
        }
        let preset = self.preset.unwrap_or(match self.algorithm {
            Some(Algorithm::Random) => Preset::Beginner,
            Some(Algorithm::Ukeire) => Preset::Strong,
            _ => Preset::Standard,
        });
        let mut value = preset.core().resolve(self.seed.value());
        if let Some(algorithm) = self.algorithm {
            value.algorithm = algorithm.core();
        }
        if let Some(n) = self.max_evaluations {
            value.max_evaluations = n;
        }
        if let Some(n) = self.lookahead_depth {
            value.lookahead_depth = n;
        }
        if let Some(n) = self.risk_weight {
            value.risk_weight = n;
        }
        value
            .validate()
            .map_err(|e| ApiError::new(ErrorCode::InvalidConfig, e.to_string()))?;
        Ok(value)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveAiConfig {
    pub algorithm: Algorithm,
    pub seed: Seed,
    pub max_evaluations: u32,
    pub lookahead_depth: u8,
    pub risk_weight: u16,
}
impl From<AiConfig> for EffectiveAiConfig {
    fn from(value: AiConfig) -> Self {
        Self {
            algorithm: match value.algorithm {
                AiAlgorithm::Random => Algorithm::Random,
                AiAlgorithm::Greedy => Algorithm::Greedy,
                AiAlgorithm::Ukeire => Algorithm::Ukeire,
            },
            seed: value.seed.into(),
            max_evaluations: value.max_evaluations,
            lookahead_depth: value.lookahead_depth,
            risk_weight: value.risk_weight,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Controller {
    Human,
    Ai {
        #[serde(default)]
        config: AiConfigSpec,
    },
}
fn default_players() -> [Controller; 4] {
    std::array::from_fn(|i| {
        if i == 0 {
            Controller::Human
        } else {
            Controller::Ai {
                config: AiConfigSpec {
                    seed: (i as u64).into(),
                    ..Default::default()
                },
            }
        }
    })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicViewer {
    Public,
}
/// TOML has no null: both transports use a seat number or the explicit string "public".
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Viewer {
    Seat(SeatId),
    Public(PublicViewer),
}
impl Viewer {
    pub fn seat(self) -> Option<SeatId> {
        match self {
            Self::Seat(seat) => Some(seat),
            Self::Public(_) => None,
        }
    }
}
fn default_viewer() -> Viewer {
    Viewer::Seat(SeatId::new(0).expect("valid default seat"))
}
fn default_limit() -> u32 {
    10_000
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GameSettings {
    #[serde(default)]
    pub seed: Seed,
    #[serde(default = "default_viewer")]
    pub viewer: Viewer,
    #[serde(default = "default_limit")]
    pub max_steps: u32,
}
impl Default for GameSettings {
    fn default() -> Self {
        Self {
            seed: Seed::default(),
            viewer: default_viewer(),
            max_steps: default_limit(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    #[default]
    Replay,
    Public,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogSettings {
    #[serde(default)]
    pub format: LogFormat,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub schema_version: Version,
    #[serde(default)]
    pub game: GameSettings,
    #[serde(default = "default_players")]
    pub players: [Controller; 4],
    #[serde(default)]
    pub logging: LogSettings,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: Version,
            game: GameSettings::default(),
            players: default_players(),
            logging: LogSettings::default(),
        }
    }
}
impl AppConfig {
    pub fn from_toml(text: &str) -> Result<Self, ApiError> {
        if text.len() > crate::MAX_REQUEST_BYTES {
            return Err(ApiError::new(
                ErrorCode::InvalidConfig,
                "configuration exceeds the byte limit",
            ));
        }
        let config: Self = toml::from_str(text)
            .map_err(|e| ApiError::new(ErrorCode::InvalidConfig, e.to_string()))?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        if !(1..=10_000).contains(&self.game.max_steps) {
            return Err(ApiError::new(
                ErrorCode::InvalidConfig,
                "max_steps must be 1..10000",
            ));
        }
        for (seat, controller) in self.players.iter().enumerate() {
            match controller {
                Controller::Human
                    if self.game.viewer.seat().map(|x| usize::from(x.get())) != Some(seat) =>
                {
                    return Err(ApiError::new(
                        ErrorCode::InvalidConfig,
                        "a human controller must be the fixed viewer",
                    ))
                }
                Controller::Ai { config } => {
                    config.resolve()?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn effective_ai(&self) -> Result<[Option<EffectiveAiConfig>; 4], ApiError> {
        let mut result = std::array::from_fn(|_| None);
        for (i, player) in self.players.iter().enumerate() {
            if let Controller::Ai { config } = player {
                result[i] = Some(config.resolve()?.into());
            }
        }
        Ok(result)
    }
}
