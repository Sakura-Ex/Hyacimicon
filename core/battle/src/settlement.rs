//! Explicit provisional business settlement, separate from combat outcome.
use crate::model::{Outcome, Scenario};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementPolicy {
    pub name: String,
    pub enemy_weights: BTreeMap<String, f64>,
    pub minimum_progress: f64,
    pub hp_before: f64,
    pub hp_cap: f64,
    pub failure_loss: f64,
    pub recovery: f64,
}

impl SettlementPolicy {
    pub fn validate(&self, scenario: &Scenario) -> Result<(), String> {
        if self.name != "weighted_kills_flat_penalty_v1" {
            return Err("unsupported settlement policy".into());
        }
        for (name, value, max) in [
            ("minimum_progress", self.minimum_progress, 1.0),
            ("hp_before", self.hp_before, self.hp_cap),
            ("hp_cap", self.hp_cap, 1e12),
            ("failure_loss", self.failure_loss, 1e12),
            ("recovery", self.recovery, 1e12),
        ] {
            if !value.is_finite() || value < 0.0 || value > max {
                return Err(format!("invalid settlement {name}"));
            }
        }
        if self.hp_cap <= 0.0 || self.hp_before <= 0.0 {
            return Err("battle requires positive run HP and cap".into());
        }
        let enemies: Vec<_> = scenario.waves.iter().flatten().collect();
        if enemies.len() != self.enemy_weights.len()
            || enemies.iter().any(|u| {
                !self
                    .enemy_weights
                    .get(&u.id)
                    .is_some_and(|w| w.is_finite() && *w > 0.0 && *w <= 1e9)
            })
        {
            return Err("settlement needs exactly one positive weight per enemy instance".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SettlementResult {
    pub status: String,
    pub policy: SettlementPolicy,
    pub defeated_ids: Vec<String>,
    pub defeated_weight: f64,
    pub total_weight: f64,
    pub progress: f64,
    pub minimum_progress_met: bool,
    pub requested_loss: Option<f64>,
    pub actual_loss: Option<f64>,
    pub requested_recovery: Option<f64>,
    pub actual_recovery: Option<f64>,
    pub loss_clipped: Option<f64>,
    pub recovery_clipped: Option<f64>,
    pub hp_after: Option<f64>,
    pub run_alive: Option<bool>,
}

/// The engine calls this once at return. Incomplete snapshots never settle HP.
pub(crate) fn settle(
    policy: &SettlementPolicy,
    defeated_ids: Vec<String>,
    outcome: Outcome,
) -> SettlementResult {
    let total_weight = policy.enemy_weights.values().sum::<f64>();
    // Use the same key order as the denominator, including when input queue order differs.
    let defeated = defeated_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let defeated_weight = policy
        .enemy_weights
        .iter()
        .filter(|(id, _)| defeated.contains(id))
        .map(|(_, weight)| *weight)
        .sum::<f64>();
    let progress = defeated_weight / total_weight;
    let minimum_progress_met = progress >= policy.minimum_progress;
    let mut result = SettlementResult {
        status: "not_applied_incomplete".into(),
        policy: policy.clone(),
        defeated_ids,
        defeated_weight,
        total_weight,
        progress,
        minimum_progress_met,
        requested_loss: None,
        actual_loss: None,
        requested_recovery: None,
        actual_recovery: None,
        loss_clipped: None,
        recovery_clipped: None,
        hp_after: None,
        run_alive: None,
    };
    if outcome != Outcome::Incomplete {
        let loss = if minimum_progress_met {
            0.0
        } else {
            policy.failure_loss
        };
        let actual_loss = loss.min(policy.hp_before);
        let after_loss = policy.hp_before - actual_loss;
        let actual_recovery = policy.recovery.min(policy.hp_cap - after_loss);
        let hp_after = after_loss + actual_recovery;
        result.status = "applied_provisional".into();
        result.requested_loss = Some(loss);
        result.actual_loss = Some(actual_loss);
        result.requested_recovery = Some(policy.recovery);
        result.actual_recovery = Some(actual_recovery);
        result.loss_clipped = Some(loss - actual_loss);
        result.recovery_clipped = Some(policy.recovery - actual_recovery);
        result.hp_after = Some(hp_after);
        result.run_alive = Some(hp_after > 0.0);
    }
    result
}
