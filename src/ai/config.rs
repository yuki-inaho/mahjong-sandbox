use crate::game::Action;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiAlgorithm {
    Random,
    Greedy,
    Ukeire,
}
impl AiAlgorithm {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Greedy => "greedy",
            Self::Ukeire => "ukeire",
        }
    }
}
impl FromStr for AiAlgorithm {
    type Err = AiError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "random" => Ok(Self::Random),
            "greedy" => Ok(Self::Greedy),
            "ukeire" => Ok(Self::Ukeire),
            _ => Err(AiError::InvalidConfig("未知のAI算法です")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiPreset {
    Beginner,
    Standard,
    Strong,
}
impl AiPreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Standard => "standard",
            Self::Strong => "strong",
        }
    }
    /// 名称は算法・予算の組を表し、勝率の順位を保証しません。
    pub const fn resolve(self, seed: u64) -> AiConfig {
        let (algorithm, max_evaluations, lookahead_depth, risk_weight) = match self {
            Self::Beginner => (AiAlgorithm::Random, 0, 0, 0),
            Self::Standard => (AiAlgorithm::Greedy, 1024, 0, 1),
            Self::Strong => (AiAlgorithm::Ukeire, 8192, 2, 4),
        };
        AiConfig {
            algorithm,
            seed,
            max_evaluations,
            lookahead_depth,
            risk_weight,
        }
    }
}
impl FromStr for AiPreset {
    type Err = AiError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "beginner" => Ok(Self::Beginner),
            "standard" => Ok(Self::Standard),
            "strong" => Ok(Self::Strong),
            _ => Err(AiError::InvalidConfig("未知のAIプリセットです")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AiConfig {
    pub algorithm: AiAlgorithm,
    pub seed: u64,
    /// 向聴計算の呼出し上限です。計算済み結果の再利用は加算しません。
    pub max_evaluations: u32,
    pub lookahead_depth: u8,
    pub risk_weight: u16,
}
impl Default for AiConfig {
    fn default() -> Self {
        AiPreset::Standard.resolve(0)
    }
}
impl AiConfig {
    pub fn validate(self) -> Result<(), AiError> {
        if self.max_evaluations > 100_000 || self.risk_weight > 1000 {
            return Err(AiError::InvalidConfig(
                "計算予算または防御重みが上限を超えています",
            ));
        }
        match self.algorithm {
            AiAlgorithm::Random
                if self.max_evaluations != 0
                    || self.lookahead_depth != 0
                    || self.risk_weight != 0 =>
            {
                Err(AiError::InvalidConfig(
                    "Randomの予算・深さ・防御重みは0です",
                ))
            }
            AiAlgorithm::Greedy if self.max_evaluations == 0 || self.lookahead_depth != 0 => Err(
                AiError::InvalidConfig("Greedyは正の予算と深さ0を使用します"),
            ),
            AiAlgorithm::Ukeire
                if self.max_evaluations == 0 || !(1..=2).contains(&self.lookahead_depth) =>
            {
                Err(AiError::InvalidConfig(
                    "Ukeireは正の予算と深さ1または2を使用します",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    Win,
    Random,
    Greedy,
    Ukeire,
    Lookahead,
    BudgetFallback,
    Forced,
}
impl DecisionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Random => "random",
            Self::Greedy => "greedy",
            Self::Ukeire => "ukeire",
            Self::Lookahead => "lookahead",
            Self::BudgetFallback => "budget_fallback",
            Self::Forced => "forced",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateEvaluation {
    pub action: Action,
    pub shanten: Option<i8>,
    pub ukeire: Option<u16>,
    pub danger: u16,
    pub lookahead_value: Option<i64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    pub action: Action,
    pub config: AiConfig,
    pub reason: DecisionReason,
    pub candidates: Vec<CandidateEvaluation>,
    pub candidates_evaluated: u32,
    pub evaluations: u32,
    pub budget_exhausted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiError {
    InvalidConfig(&'static str),
    InvalidObservation(&'static str),
    InvalidAction(&'static str),
    NoLegalActions,
}
impl fmt::Display for AiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message)
            | Self::InvalidObservation(message)
            | Self::InvalidAction(message) => formatter.write_str(message),
            Self::NoLegalActions => formatter.write_str("合法手がありません"),
        }
    }
}
impl std::error::Error for AiError {}
