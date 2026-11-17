//! Automatic and scripted callers select from the same currently legal operations.
use crate::model::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    pub actor: usize,
    pub action: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionPoint {
    Insert,
    NormalTurn,
    ExtraTurn,
}
#[derive(Clone, Debug)]
pub struct AxisPosition {
    pub id: String,
    pub remaining_distance: f64,
    pub speed: f64,
    pub waiting_av: f64,
}
pub struct Observation {
    pub now: f64,
    pub cycle: usize,
    pub cycle_remaining_av: f64,
    pub axis: Vec<AxisPosition>,
    pub units: Vec<UnitResult>,
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
