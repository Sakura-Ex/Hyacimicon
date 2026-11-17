//! 单局、单位与回放记录的输出格式。
use super::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Victory,
    Defeat,
    Draw,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    EnemiesDefeated,
    AlliesDefeated,
    BothDefeated,
    AvBudgetExhausted,
    ActionLimit,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitResult {
    /// None means no natural-weakness input, not an empty natural weakness set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weaknesses: Option<crate::weakness::WeaknessState>,
    pub id: String,
    pub ally: bool,
    pub hp: f64,
    pub shield: f64,
    pub energy: f64,
    pub alive: bool,
    pub revivals_left: u8,
    pub remaining_distance: f64,
    pub speed: f64,
    pub toughness: f64,
    pub dynamic_values: std::collections::BTreeMap<String, f64>,
    pub modifiers: Vec<crate::modifiers::ModifierState>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub index: u32,
    pub av: f64,
    pub kind: String,
    pub actor: String,
    pub target: Option<String>,
    pub action: Option<String>,
    pub amount: Option<f64>,
    pub critical: Option<bool>,
    pub attack_type: Option<AttackType>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueueResult {
    pub policy: String,
    pub capacity: usize,
    pub active_slots: Vec<Option<String>>,
    pub remaining_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BattleResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<QueueResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement: Option<crate::settlement::SettlementResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    pub provenance: Provenance,
    pub scenario_sha256: String,
    #[serde(serialize_with = "seed_decimal::serialize")]
    pub seed: u64,
    pub ruleset: String,
    pub outcome: Outcome,
    pub termination: Termination,
    pub elapsed_av: f64,
    pub cycle: usize,
    pub cycle_remaining_av: f64,
    pub remaining_av: f64,
    pub penalty_av: f64,
    pub actions: u32,
    pub waves_cleared: usize,
    pub ally_skill_points: u8,
    pub enemy_skill_points: u8,
    pub units: Vec<UnitResult>,
    pub events: Vec<Event>,
    pub trace_truncated: bool,
    /// Actual loss under the explicit settlement policy; None for legacy/incomplete.
    pub run_hp_loss: Option<f64>,
    pub validation_scope: String,
}

/// JSON transports full-width seeds as strings so TypeScript cannot round them.
pub mod seed_decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty() || !text.bytes().all(|c| c.is_ascii_digit()) {
            return Err(serde::de::Error::custom(
                "seed must be a decimal u64 string",
            ));
        }
        text.parse().map_err(serde::de::Error::custom)
    }
}
