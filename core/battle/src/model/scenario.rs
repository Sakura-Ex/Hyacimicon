//! 场景、显式机制参数和校验后输入。
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub game_version: String,
    pub data_snapshot: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Explicit effective units, actions, resources and limits for a `battle_v2` battle.
pub struct Scenario {
    #[serde(default)]
    pub mechanics: Mechanics,
    pub schema_version: u32,
    /// The only implemented profile is battle_v2; it is not a full character catalog.
    pub ruleset: String,
    pub provenance: Provenance,
    pub unsupported_mechanics: Vec<String>,
    /// Sum of cycle allowances; current-cycle-capped revival charges consume it.
    /// Exhaustion yields
    /// [`Outcome::Defeat`] / [`Termination::AvBudgetExhausted`], unless terminal
    /// elimination has already settled. Equal-time order is an explicit mechanics policy.
    pub max_av: f64,
    /// Maximum executed actions, including ultimates, shared across waves.
    /// Reaching this safety limit yields [`Outcome::Incomplete`] /
    /// [`Termination::ActionLimit`] after terminal, elapsed-budget and cancellation checks.
    pub max_actions: u32,
    pub ally_skill_points: SkillPoints,
    pub enemy_skill_points: SkillPoints,
    pub allies: Vec<Unit>,
    pub waves: Vec<Vec<Unit>>,
}

/// Constructed only after validation. Callers cannot mutate the validated input.
#[derive(Clone, Debug)]
pub struct ValidatedScenario {
    pub(crate) scenario: Scenario,
    pub(crate) fingerprint: String,
}

impl ValidatedScenario {
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mechanics {
    /// Some enables a finite queue in `waves[0]`; remaining waves must be absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_capacity: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<crate::settlement::SettlementPolicy>,
    /// Caller-declared approximation/configuration ledger, echoed in all outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// Explicit immutable natural weaknesses. Missing units remain unknown.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub natural_weaknesses: std::collections::BTreeMap<String, Vec<crate::elements::Element>>,
    /// Weakness contributions owned by the corresponding active modifier instance.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub modifier_weaknesses: std::collections::BTreeMap<String, Vec<crate::elements::Element>>,
    #[serde(default)]
    pub element_profiles: std::collections::BTreeMap<String, crate::elements::ElementProfile>,
    #[serde(default)]
    pub modifier_definitions:
        std::collections::BTreeMap<String, crate::modifiers::ModifierDefinition>,
    /// Explicit initial actor variables; missing runtime reads are errors.
    #[serde(default)]
    pub dynamic_values: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    /// Empty uses one cycle of max_av. Otherwise their sum must equal max_av.
    #[serde(default)]
    pub cycles: Vec<f64>,
    #[serde(default = "one")]
    pub ally_damage_factor: f64,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub toughness: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    pub ultimate_priority: std::collections::BTreeMap<String, i32>,
    /// Explicit tie policy pending game observation; never inferred from numeric values.
    #[serde(default)]
    pub larger_priority_first: bool,
    #[serde(default)]
    pub unit_before_cycle: bool,
}

fn one() -> f64 {
    1.0
}

impl Default for Mechanics {
    fn default() -> Self {
        Self {
            queue_capacity: None,
            settlement: None,
            assumptions: vec![],
            natural_weaknesses: Default::default(),
            modifier_weaknesses: Default::default(),
            element_profiles: Default::default(),
            modifier_definitions: Default::default(),
            dynamic_values: Default::default(),
            cycles: vec![],
            ally_damage_factor: 1.0,
            triggers: vec![],
            toughness: Default::default(),
            ultimate_priority: Default::default(),
            larger_priority_first: false,
            unit_before_cycle: false,
        }
    }
}
