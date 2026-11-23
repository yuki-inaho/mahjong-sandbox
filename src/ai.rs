//! 公開観測と合法手だけを受け取るCPU。設定と判断の根拠を値として返します。
mod config;
mod search;
mod validation;

#[cfg(test)]
mod tests;
use crate::game::{Action, Observation};

/// 構造と公開枚数を検査し、未公開の各牌の枚数を返します。
/// 他家の手牌・王牌も未公開枚数に含まれ、実際の山の残枚数ではありません。
/// 外部Policyも同じ検査と計数を再利用できます。
pub fn unseen_tile_counts(
    observation: &Observation,
    legal: &[Action],
) -> Result<[u8; 34], AiError> {
    validation::validate(observation, legal).map(|visible| visible.map(|amount| 4 - amount))
}
pub use config::{
    AiAlgorithm, AiConfig, AiError, AiPreset, CandidateEvaluation, Decision, DecisionReason,
};
/// 木探索・学習済みモデルにも使える差替え点。対局状態は変更しません。
pub trait Policy {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action>;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredAi {
    config: AiConfig,
}
impl ConfiguredAi {
    pub fn new(config: AiConfig) -> Result<Self, AiError> {
        config.validate()?;
        Ok(Self { config })
    }
    pub fn config(&self) -> AiConfig {
        self.config
    }
    pub fn decide(&self, observation: &Observation, legal: &[Action]) -> Result<Decision, AiError> {
        search::decide(self.config, observation, legal)
    }
}
impl Policy for ConfiguredAi {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action> {
        self.decide(observation, legal).ok().map(|d| d.action)
    }
}
/// 初版からの向聴数・公開河・牌のつながりによる判断方式です。
#[derive(Clone, Copy, Debug, Default)]
pub struct GreedyAi;
impl Policy for GreedyAi {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action> {
        ConfiguredAi::new(AiPreset::Standard.resolve(0))
            .ok()?
            .choose_action(observation, legal)
    }
}
