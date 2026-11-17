//! Automatic and scripted callers select from the same currently legal operations.
use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub actor: usize,
    pub action: usize,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionPoint {
    Insert,
    NormalTurn,
    ExtraTurn,
}
#[derive(Clone, Debug, Serialize)]
pub struct AxisPosition {
    pub id: String,
    pub remaining_distance: f64,
    pub speed: f64,
    pub waiting_av: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Observation {
    pub now: f64,
    pub cycle: usize,
    pub cycle_remaining_av: f64,
    /// 当前Cycle及后续Cycle的剩余总AV，供外部策略分配预算。
    pub remaining_av: f64,
    pub ally_skill_points: u8,
    pub enemy_skill_points: u8,
    pub axis: Vec<AxisPosition>,
    pub units: Vec<UnitResult>,
}

/// 一个暂停点的观测与合法选项，供自动策略、模型推理和训练读取。
///
/// `id`在单个会话内递增；提交时携带它，避免旧决策或重复提交影响新局面。
/// `None`表示放弃本次插入机会，仅在`Insert`决策点允许。
#[derive(Clone, Debug, Serialize)]
pub struct Decision {
    pub id: u64,
    pub point: DecisionPoint,
    pub observation: Observation,
    pub legal: Vec<Operation>,
}
/// Return a member of legal, or None to decline inserts. A turn requires a choice.
pub trait Policy {
    fn decide(
        &mut self,
        point: DecisionPoint,
        state: &Observation,
        legal: &[Operation],
    ) -> Option<Operation>;
}
pub struct Automatic;
impl Policy for Automatic {
    fn decide(
        &mut self,
        _: DecisionPoint,
        _: &Observation,
        legal: &[Operation],
    ) -> Option<Operation> {
        legal.first().cloned()
    }
}
/// Script closure receives the same observation and legal list as Automatic.
impl<F> Policy for F
where
    F: FnMut(DecisionPoint, &Observation, &[Operation]) -> Option<Operation>,
{
    fn decide(&mut self, p: DecisionPoint, s: &Observation, l: &[Operation]) -> Option<Operation> {
        self(p, s, l)
    }
}
