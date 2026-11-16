//! Explicit synthetic run inputs and records; no live-game settlement is inferred.
use battle_core::model::{BattleResult, Outcome, Provenance, Scenario, Termination};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub hp: f64,
    pub max_hp: f64,
    pub gold: u32,
    pub crystals: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDelta {
    pub hp: f64,
    /// Signed values are constrained to +/- u32::MAX for exact JS transport.
    pub gold: i64,
    pub crystals: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementRules {
    /// Only synthetic_explicit_v1 is implemented. Missing branches remain unknown.
    pub ruleset: String,
    pub rule_id: String,
    pub source: String,
    pub victory: Option<ResourceDelta>,
    pub defeat: Option<ResourceDelta>,
    pub draw: Option<ResourceDelta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    pub id: String,
    /// Each node uses its explicit initial battle state; no cross-node unit reset
    /// or carryover is inferred from the run HP and economic balances.
    pub battle: Scenario,
    pub settlement: Option<SettlementRules>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    pub schema_version: u32,
    pub ruleset: String,
    /// Explicit caller-provided identity; this does not perform account adaptation.
    pub account_config_id: String,
    pub provenance: Provenance,
    pub initial: Resources,
    pub nodes: Vec<NodeSpec>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Ready,
    AwaitingSettlement,
    Completed,
    Defeated,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunAction {
    FightCurrentNode,
}

/// Policy-facing data only: no future nodes, full scenarios, seeds, or replay.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct RunObservation {
    pub run_id: String,
    pub account_config_id: String,
    pub revision: u32,
    pub node_index: u32,
    pub current_node_id: Option<String>,
    pub resources: Resources,
    pub status: RunStatus,
    pub ruleset: String,
    pub legal_actions: Vec<RunAction>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SampleRequest {
    pub request_id: String,
    pub node_id: String,
    pub expected_revision: u32,
    pub trace: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementRequest {
    pub request_id: String,
    pub node_id: String,
    pub attempt_id: String,
    pub expected_revision: u32,
    pub rules: SettlementRules,
}

#[derive(Clone, Debug, Serialize)]
pub struct BattleDiagnostic {
    pub budget_limit_av: f64,
    pub budget_used_av: f64,
    pub budget_margin_av: f64,
    pub remaining_av: f64,
    pub overrun_av: f64,
    pub success_remaining_av: Option<f64>,
    /// The known engine termination only, never an inferred causal explanation.
    pub failure_reason: Option<Termination>,
    pub incomplete_reason: Option<Termination>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BattleResolution {
    pub result: BattleResult,
    pub diagnostic: BattleDiagnostic,
}

#[derive(Clone, Debug, Serialize)]
pub struct BattleAttempt {
    pub attempt_id: String,
    pub request_id: String,
    pub node_id: String,
    pub before: RunObservation,
    pub sampled_revision: u32,
    pub resolution: BattleResolution,
    /// Run-spec provenance, distinct from this node's battle data.
    pub provenance: Provenance,
    pub battle_provenance: Provenance,
    pub origin: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SettlementReceipt {
    pub request_id: String,
    pub attempt_id: String,
    pub node_id: String,
    pub expected_revision: u32,
    pub outcome: Outcome,
    pub rules: SettlementRules,
    pub rules_sha256: String,
    pub requested_delta: ResourceDelta,
    pub applied_delta: ResourceDelta,
    pub before: RunObservation,
    pub after: RunObservation,
}

/// Every committed loss is retained, including losses that continue the episode.
#[derive(Clone, Debug, Serialize)]
pub struct TrainingTransition {
    pub before: RunObservation,
    pub action: RunAction,
    pub attempt: BattleAttempt,
    pub settlement: SettlementReceipt,
    pub after: RunObservation,
    pub terminated: bool,
    pub truncated: bool,
    /// No scalar reward has been specified for this foundation.
    pub reward: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StepResult {
    pub attempt: BattleAttempt,
    pub settlement: Option<SettlementReceipt>,
    pub observation: RunObservation,
    pub pending_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunReport {
    pub spec_sha256: String,
    #[serde(serialize_with = "battle_core::model::seed_decimal::serialize")]
    pub master_seed: u64,
    pub seed_derivation: String,
    pub observation: RunObservation,
    pub attempts: Vec<BattleAttempt>,
    pub settlements: Vec<SettlementReceipt>,
    pub transitions: Vec<TrainingTransition>,
    pub scope: String,
}
