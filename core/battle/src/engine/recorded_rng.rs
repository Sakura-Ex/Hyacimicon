//! Strict replay of observed random branches without overriding combat values.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordedDraw {
    /// None spans initialization and the advance to the first decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step: Option<usize>,
    actor: String,
    action: String,
    result: RecordedResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RecordedResult {
    Critical { target: String, critical: bool },
    UniformEnemy { target: String },
    TauntEnemy { target: String },
    HitRetarget { target: String },
}

impl RecordedResult {
    fn kind(&self) -> &'static str {
        match self {
            Self::Critical { .. } => "critical",
            Self::UniformEnemy { .. } => "uniform_enemy",
            Self::TauntEnemy { .. } => "taunt_enemy",
            Self::HitRetarget { .. } => "hit_retarget",
        }
    }

    pub(super) fn target(&self) -> &str {
        match self {
            Self::Critical { target, .. }
            | Self::UniformEnemy { target }
            | Self::TauntEnemy { target }
            | Self::HitRetarget { target } => target,
        }
    }
}

pub(super) struct RecordedRandom {
    draws: VecDeque<RecordedDraw>,
    step: Option<usize>,
    consumed: usize,
}

impl RecordedRandom {
    pub(super) fn new(draws: Vec<RecordedDraw>) -> Self {
        Self {
            draws: draws.into(),
            step: None,
            consumed: 0,
        }
    }

    pub(super) fn set_step(&mut self, step: usize) {
        self.step = Some(step);
    }

    pub(super) fn take(
        &mut self,
        kind: &str,
        actor: &str,
        action: &str,
        hit_target: Option<&str>,
    ) -> Result<RecordedResult, String> {
        let index = self.consumed;
        let expected = format!("{kind} from {actor:?}/{action:?} for {hit_target:?}");
        let draw = self
            .draws
            .front()
            .ok_or_else(|| format!("missing random outcome {index}: {expected}"))?;
        if draw.step > self.step {
            return Err(format!(
                "missing random outcome {index}: {expected}; next record is for step {:?}",
                draw.step
            ));
        }
        if draw.step != self.step
            || draw.result.kind() != kind
            || draw.actor != actor
            || draw.action != action
            || hit_target.is_some_and(|target| draw.result.target() != target)
        {
            return Err(format!(
                "random outcome {index} mismatch: expected {expected}, got {draw:?}"
            ));
        }
        let draw = self
            .draws
            .pop_front()
            .ok_or("random record disappeared during consumption")?;
        self.consumed += 1;
        Ok(draw.result)
    }

    /// Reject records left in the finished interval, or any leftover at terminal.
    pub(super) fn check_remaining(&self, terminal: bool) -> Result<(), String> {
        if let Some(next) = self.draws.front() {
            if terminal || next.step <= self.step {
                return Err(format!("unused random outcome {}: {next:?}", self.consumed));
            }
        }
        Ok(())
    }
}
