//! Decision explanations are only exposed for the requesting player's own turn.
use crate::{
    config::EffectiveAiConfig,
    types::{SignedValue, WireAction},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use suzume::ai::{Decision, DecisionReason};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Win,
    Random,
    Greedy,
    Ukeire,
    Lookahead,
    BudgetFallback,
    Forced,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CandidateView {
    pub action: WireAction,
    pub shanten: Option<i8>,
    pub ukeire: Option<u16>,
    pub danger: u16,
    pub lookahead_value: Option<SignedValue>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionView {
    pub action: WireAction,
    pub config: EffectiveAiConfig,
    pub reason: Reason,
    pub candidates: Vec<CandidateView>,
    pub candidates_evaluated: u32,
    pub evaluations: u32,
    pub budget_exhausted: bool,
}
impl DecisionView {
    pub fn from_core(value: &Decision) -> Option<Self> {
        let candidates = value
            .candidates
            .iter()
            .map(|c| {
                Some(CandidateView {
                    action: WireAction::from_core(&c.action)?,
                    shanten: c.shanten,
                    ukeire: c.ukeire,
                    danger: c.danger,
                    lookahead_value: c.lookahead_value.map(Into::into),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            action: WireAction::from_core(&value.action)?,
            config: value.config.into(),
            reason: match value.reason {
                DecisionReason::Win => Reason::Win,
                DecisionReason::Random => Reason::Random,
                DecisionReason::Greedy => Reason::Greedy,
                DecisionReason::Ukeire => Reason::Ukeire,
                DecisionReason::Lookahead => Reason::Lookahead,
                DecisionReason::BudgetFallback => Reason::BudgetFallback,
                DecisionReason::Forced => Reason::Forced,
            },
            candidates,
            candidates_evaluated: value.candidates_evaluated,
            evaluations: value.evaluations,
            budget_exhausted: value.budget_exhausted,
        })
    }
}
