//! Validated input and result contracts shared by the CLI and library callers.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Effective values, already composed by the caller. No account data is guessed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub level: u16,
    pub max_hp: f64,
    pub attack: f64,
    pub defense: f64,
    pub speed: f64,
    pub taunt: f64,
    pub crit_rate: f64,
    pub crit_damage: f64,
    pub damage_bonus: f64,
    pub defense_reduction: f64,
    pub defense_ignore: f64,
    pub resistance: f64,
    pub resistance_penetration: f64,
    pub vulnerability: f64,
    pub damage_reduction: f64,
    pub independent_multiplier: f64,
    pub toughness_broken: bool,
    pub healing_bonus: f64,
    pub incoming_healing_bonus: f64,
    pub shield_bonus: f64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    Attack,
    Defense,
    MaxHp,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// One target explicitly bound by a Retarget effect; nested scopes restore it.
    ContextTarget,
    /// Select lowest HP fraction once at Action entry and retain it for every hit.
    AbilityTarget,
    EnemyLowestHp,
    EnemyRandomTaunt,
    EnemyRandomUniform,
    AllEnemies,
    AllyLowestHp,
    AllAllies,
    SelfUnit,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    /// A fresh invocation frame, removed on return even after a runtime error.
    WithDynamicScope {
        scope: crate::dynamic_scope::DynamicScope,
        effects: Vec<Effect>,
    },
    SetScopedDynamicValue {
        scope: crate::dynamic_scope::DynamicScope,
        key: String,
        value: crate::expression::Expression,
    },
    /// One sourced hit with explicit element panels; no implicit toughness effect.
    ElementalDamage {
        scale: Scale,
        ratio: crate::expression::Expression,
        flat: crate::expression::Expression,
        element: crate::elements::DamageElement,
        attack_type: AttackType,
        can_crit: bool,
        target: Target,
    },
    /// Source DamagePerformFinish boundary, distinct from completion of Action.
    AttackFinish,
    RetargetAttackRandom {
        effects: Vec<Effect>,
        empty_effects: Vec<Effect>,
    },
    AddModifier {
        id: String,
        stacks: crate::expression::Expression,
        target: Target,
    },
    RemoveModifier {
        id: String,
        from_any_source: bool,
        target: Target,
    },
    Conditional {
        condition: Condition,
        then_effects: Vec<Effect>,
        else_effects: Vec<Effect>,
    },
    Repeat {
        count: crate::expression::Expression,
        effects: Vec<Effect>,
    },
    SetDynamicValue {
        key: String,
        value: crate::expression::Expression,
        target: Target,
    },
    DynamicEnergy {
        amount: crate::expression::Expression,
        target: Target,
    },
    Damage {
        scale: Scale,
        ratio: f64,
        flat: f64,
        hits: u8,
        can_crit: bool,
        target: Target,
    },
    Heal {
        scale: Scale,
        ratio: f64,
        flat: f64,
        target: Target,
    },
    Shield {
        scale: Scale,
        ratio: f64,
        flat: f64,
        max_total: f64,
        duration_turns: u16,
        target: Target,
    },
    Energy {
        amount: f64,
        target: Target,
    },
    Speed {
        value: f64,
        target: Target,
    },
    AdvanceNext {
        fraction: f64,
        target: Target,
    },
    Toughness {
        amount: f64,
        target: Target,
    },
    Request {
        kind: RequestKind,
        action: Option<String>,
        target: Target,
    },
    Advance {
        fraction: f64,
        target: Target,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// Only self, bound ability target or any living enemy; never consumes RNG.
    HasModifier {
        id: String,
        target: Target,
    },
    HitAttackType {
        attack_type: AttackType,
    },
    Compare {
        left: crate::expression::Expression,
        comparison: Comparison,
        right: crate::expression::Expression,
    },
    ActionKind {
        kind: ActionKind,
    },
    Not {
        condition: Box<Condition>,
    },
    All {
        conditions: Vec<Condition>,
    },
    Any {
        conditions: Vec<Condition>,
    },
}
impl Condition {
    fn validate_modifier_references(
        &self,
        definitions: &std::collections::BTreeMap<String, crate::modifiers::ModifierDefinition>,
    ) -> Result<(), String> {
        match self {
            Self::HasModifier { id, .. } if !definitions.contains_key(id) => {
                Err(format!("unknown condition modifier {id}"))
            }
            Self::Not { condition } => condition.validate_modifier_references(definitions),
            Self::All { conditions } | Self::Any { conditions } => {
                for condition in conditions {
                    condition.validate_modifier_references(definitions)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("condition nesting exceeds 32".into());
        }
        match self {
            Self::Compare { left, right, .. } => {
                left.validate()?;
                right.validate()?;
            }
            Self::HasModifier { id, target } => {
                identifier(id, "condition modifier")?;
                if !matches!(
                    target,
                    Target::SelfUnit | Target::AbilityTarget | Target::AllEnemies
                ) {
                    return Err(
                        "modifier condition requires self, ability_target or all_enemies".into(),
                    );
                }
            }
            Self::ActionKind { .. } => {}
            Self::HitAttackType { .. } => {}
            Self::Not { condition } => condition.validate(depth + 1)?,
            Self::All { conditions } | Self::Any { conditions } => {
                if conditions.is_empty() || conditions.len() > 32 {
                    return Err("condition list must contain 1..32 entries".into());
                }
                for c in conditions {
                    c.validate(depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

/// All nested branches are validated, including currently unreachable branches.
fn flattened_effects(effects: &[Effect]) -> Result<Vec<&Effect>, String> {
    fn visit<'a>(
        effects: &'a [Effect],
        depth: usize,
        out: &mut Vec<&'a Effect>,
    ) -> Result<(), String> {
        if depth > 32 {
            return Err("effect nesting exceeds 32".into());
        }
        for effect in effects {
            if out.len() >= 4096 {
                return Err("effect tree exceeds 4096 nodes".into());
            }
            out.push(effect);
            match effect {
                Effect::WithDynamicScope { effects, .. } => visit(effects, depth + 1, out)?,
                Effect::Conditional {
                    then_effects,
                    else_effects,
                    ..
                } => {
                    visit(then_effects, depth + 1, out)?;
                    visit(else_effects, depth + 1, out)?;
                }
                Effect::RetargetAttackRandom {
                    effects,
                    empty_effects,
                } => {
                    visit(effects, depth + 1, out)?;
                    visit(empty_effects, depth + 1, out)?;
                }
                Effect::Repeat { effects, .. } => visit(effects, depth + 1, out)?,
                _ => {}
            }
        }
        Ok(())
    }
    let mut out = vec![];
    visit(effects, 0, &mut out)?;
    Ok(out)
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Basic,
    Skill,
    Ultimate,
    FollowUp,
}
/// Hit classification is independent of the enclosing skill/action kind.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttackType {
    Direct,
    Pursued,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub kind: ActionKind,
    pub sp_cost: u8,
    pub sp_gain: u8,
    pub energy_cost: f64,
    pub energy_gain: f64,
    pub effects: Vec<Effect>,
}
/// Optional explicit model of immediate revival. It is not inferred from game data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Revival {
    pub charges: u8,
    pub hp_fraction: f64,
    pub av_penalty: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub id: String,
    pub stats: Stats,
    pub initial_hp: f64,
    pub initial_energy: f64,
    pub max_energy: f64,
    pub revival: Option<Revival>,
    /// First affordable action wins, with ultimates considered at action boundaries.
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPoints {
    pub initial: u8,
    pub maximum: u8,
}
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

fn bounded(value: f64, min: f64, max: f64, name: &str) -> Result<(), String> {
    if !value.is_finite() || value < min || value > max {
        return Err(format!("invalid {name}: must be finite in [{min}, {max}]"));
    }
    Ok(())
}
fn identifier(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 200 || value.trim() != value {
        return Err(format!("invalid {name}"));
    }
    Ok(())
}
fn validate_unit(unit: &Unit, sp: &SkillPoints) -> Result<(), String> {
    identifier(&unit.id, "unit id")?;
    let s = &unit.stats;
    if s.level == 0 || s.level > 1000 {
        return Err("invalid level".into());
    }
    for (value, name) in [(s.max_hp, "max_hp"), (s.speed, "speed"), (s.taunt, "taunt")] {
        bounded(value, 0.000001, 1e12, name)?;
    }
    for (value, name) in [
        (s.attack, "attack"),
        (s.defense, "defense"),
        (s.crit_damage, "crit_damage"),
        (s.defense_reduction, "defense_reduction"),
        (s.defense_ignore, "defense_ignore"),
        (s.independent_multiplier, "independent_multiplier"),
    ] {
        bounded(value, 0.0, 1e12, name)?;
    }
    for (value, name) in [
        (s.healing_bonus, "healing_bonus"),
        (s.incoming_healing_bonus, "incoming_healing_bonus"),
        (s.shield_bonus, "shield_bonus"),
    ] {
        bounded(value, -1.0, 1e12, name)?;
    }
    for (value, name) in [
        (s.damage_bonus, "damage_bonus"),
        (s.resistance, "resistance"),
        (s.resistance_penetration, "resistance_penetration"),
        (s.vulnerability, "vulnerability"),
    ] {
        bounded(value, -1.0, 1e6, name)?;
    }
    bounded(s.crit_rate, 0.0, 1.0, "crit_rate")?;
    bounded(s.damage_reduction, 0.0, 1.0, "damage_reduction")?;
    bounded(unit.initial_hp, 0.000001, s.max_hp, "initial_hp")?;
    bounded(unit.max_energy, 0.0, 1e9, "max_energy")?;
    bounded(unit.initial_energy, 0.0, unit.max_energy, "initial_energy")?;
    if let Some(r) = &unit.revival {
        bounded(r.hp_fraction, 0.000001, 1.0, "revival.hp_fraction")?;
        bounded(r.av_penalty, 0.0, 1e12, "revival.av_penalty")?;
    }
    if unit.actions.is_empty() || unit.actions.len() > 16 {
        return Err("actions must contain 1..16 entries".into());
    }
    let mut ids = BTreeSet::new();
    let mut fallback = false;
    for a in &unit.actions {
        identifier(&a.id, "action id")?;
        if !ids.insert(&a.id) {
            return Err("duplicate action id".into());
        }
        if a.sp_cost > sp.maximum || a.sp_gain > sp.maximum {
            return Err("action skill points exceed team maximum".into());
        }
        bounded(a.energy_cost, 0.0, unit.max_energy, "energy_cost")?;
        bounded(a.energy_gain, 0.0, 1e9, "energy_gain")?;
        if a.kind == ActionKind::Ultimate && a.energy_cost <= 0.0 {
            return Err("ultimate must cost positive energy".into());
        }
        if matches!(a.kind, ActionKind::Basic | ActionKind::Skill)
            && a.sp_cost == 0
            && a.energy_cost == 0.0
        {
            fallback = true;
        }
        if a.effects.is_empty() || a.effects.len() > 32 {
            return Err("effects must contain 1..32 entries".into());
        }
        for e in flattened_effects(&a.effects)? {
            match e {
                Effect::ElementalDamage {
                    ratio,
                    flat,
                    target,
                    ..
                } => {
                    ratio.validate()?;
                    flat.validate()?;
                    if !matches!(
                        target,
                        Target::AbilityTarget
                            | Target::ContextTarget
                            | Target::EnemyLowestHp
                            | Target::EnemyRandomTaunt
                            | Target::EnemyRandomUniform
                            | Target::AllEnemies
                    ) {
                        return Err("elemental damage requires enemy targets".into());
                    }
                }
                Effect::AttackFinish
                | Effect::RetargetAttackRandom { .. }
                | Effect::WithDynamicScope { .. } => {}
                Effect::AddModifier { id, stacks, .. } => {
                    identifier(id, "modifier id")?;
                    stacks.validate()?;
                }
                Effect::RemoveModifier { id, .. } => identifier(id, "modifier id")?,
                Effect::Conditional { condition, .. } => condition.validate(0)?,
                Effect::Repeat { count, .. } => count.validate()?,
                Effect::SetDynamicValue { key, value, .. }
                | Effect::SetScopedDynamicValue { key, value, .. } => {
                    identifier(key, "dynamic key")?;
                    value.validate()?;
                }
                Effect::DynamicEnergy { amount, .. } => amount.validate()?,
                Effect::Damage {
                    ratio,
                    flat,
                    hits,
                    target,
                    ..
                } => {
                    bounded(*ratio, 0.0, 1e6, "damage ratio")?;
                    bounded(*flat, 0.0, 1e12, "damage flat")?;
                    if *hits == 0 || *hits > 100 {
                        return Err("damage hits must be 1..100".into());
                    }
                    if !matches!(
                        target,
                        Target::ContextTarget
                            | Target::AbilityTarget
                            | Target::EnemyLowestHp
                            | Target::EnemyRandomTaunt
                            | Target::EnemyRandomUniform
                            | Target::AllEnemies
                    ) {
                        return Err("damage requires enemy targets".into());
                    }
                }
                Effect::Heal {
                    ratio,
                    flat,
                    target,
                    ..
                }
                | Effect::Shield {
                    ratio,
                    flat,
                    target,
                    ..
                } => {
                    bounded(*ratio, 0.0, 1e6, "support ratio")?;
                    bounded(*flat, 0.0, 1e12, "support flat")?;
                    if !matches!(
                        target,
                        Target::AllyLowestHp | Target::AllAllies | Target::SelfUnit
                    ) {
                        return Err("support requires ally targets".into());
                    }
                    if let Effect::Shield {
                        max_total,
                        duration_turns,
                        ..
                    } = e
                    {
                        bounded(*max_total, 0.0, 1e15, "shield max_total")?;
                        if *duration_turns == 0 {
                            return Err("shield duration_turns must be positive".into());
                        }
                    }
                }
                Effect::Energy { amount, .. } => bounded(*amount, -1e9, 1e9, "energy amount")?,
                Effect::Speed { value, .. } => bounded(*value, 0.000001, 1e12, "speed")?,
                Effect::Toughness { amount, .. } => bounded(*amount, 0.0, 1e12, "toughness")?,
                Effect::Request { kind, action, .. } => {
                    if (*kind == RequestKind::ExtraTurn) != action.is_none() {
                        return Err(
                            "extra turn selects at entry; other requests require action".into()
                        );
                    }
                }
                Effect::AdvanceNext { fraction, .. } | Effect::Advance { fraction, .. } => {
                    bounded(*fraction, -1.0, 1.0, "advance fraction")?
                }
            }
        }
    }
    if !fallback {
        return Err("unit requires a free non-ultimate fallback action".into());
    }
    Ok(())
}
/// Validate every potential healing pair on one coexisting team. Target choice
/// may change with HP and deaths, so initial HP alone cannot narrow recipients.
fn validate_healing_targets(team: &[Unit]) -> Result<(), String> {
    for actor in team {
        for action in &actor.actions {
            for effect in flattened_effects(&action.effects)? {
                if let Effect::Heal { target, .. } = effect {
                    let recipients = match target {
                        Target::SelfUnit => std::slice::from_ref(actor),
                        Target::AllyLowestHp | Target::AllAllies => team,
                        _ => return Err("support requires ally targets".into()),
                    };
                    for recipient in recipients {
                        crate::math::heal(
                            1.0,
                            actor.stats.healing_bonus,
                            recipient.stats.incoming_healing_bonus,
                        )
                        .map_err(|error| {
                            format!(
                                "invalid healing from {} to {}: {error}",
                                actor.id, recipient.id
                            )
                        })?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Validate once before running any trials; unsupported mechanisms fail closed.
pub fn validate(scenario: Scenario) -> Result<ValidatedScenario, String> {
    use sha2::{Digest, Sha256};
    if scenario.schema_version != 1 || scenario.ruleset != "battle_v2" {
        return Err("unsupported schema_version or ruleset".into());
    }
    for (value, name) in [
        (&scenario.provenance.game_version, "game_version"),
        (&scenario.provenance.data_snapshot, "data_snapshot"),
        (&scenario.provenance.description, "description"),
    ] {
        identifier(value, name)?;
    }
    if !scenario.unsupported_mechanics.is_empty() {
        return Err(format!(
            "unsupported mechanics: {}",
            scenario.unsupported_mechanics.join(", ")
        ));
    }
    bounded(scenario.max_av, 0.000001, 1e9, "max_av")?;
    if scenario.max_actions == 0 || scenario.max_actions > 1_000_000 {
        return Err("max_actions must be 1..1000000".into());
    }
    for sp in [&scenario.ally_skill_points, &scenario.enemy_skill_points] {
        if sp.maximum > 20 || sp.initial > sp.maximum {
            return Err("invalid team skill point limits".into());
        }
    }
    if scenario.allies.is_empty()
        || scenario.allies.len() > 32
        || scenario.waves.is_empty()
        || scenario.waves.len() > 100
    {
        return Err("invalid allies/wave count".into());
    }
    let mut ids = BTreeSet::new();
    for u in &scenario.allies {
        validate_unit(u, &scenario.ally_skill_points)?;
        if !ids.insert(&u.id) {
            return Err("unit ids must be globally unique".into());
        }
    }
    for wave in &scenario.waves {
        if wave.is_empty()
            || wave.len()
                > if scenario.mechanics.queue_capacity.is_some() {
                    3200
                } else {
                    32
                }
        {
            return Err("enemy wave must contain 1..32 units (finite queue: 1..3200)".into());
        }
        for u in wave {
            validate_unit(u, &scenario.enemy_skill_points)?;
            if !ids.insert(&u.id) {
                return Err("unit ids must be globally unique".into());
            }
        }
    }
    validate_mechanics(&scenario)?;
    validate_healing_targets(&scenario.allies)?;
    for wave in &scenario.waves {
        validate_healing_targets(wave)?;
    }
    let encoded = serde_json::to_vec(&scenario).map_err(|e| e.to_string())?;
    Ok(ValidatedScenario {
        scenario,
        fingerprint: format!("{:x}", Sha256::digest(encoded)),
    })
}

/// Runtime events are dispatched even when replay recording is disabled.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattleEvent {
    BattleStart,
    WaveStart,
    TurnStart,
    TurnEnd,
    ActionStart,
    BeforeSkillUse,
    AfterAttack,
    ActionEnd,
    Hit,
    Break,
    Death,
    CycleEnd,
}
impl BattleEvent {
    /// Global events have no unit subject and tick every present owner's lifetime.
    pub fn is_global(self) -> bool {
        matches!(self, Self::BattleStart | Self::WaveStart | Self::CycleEnd)
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Ultimate,
    ExtraTurn,
    FollowUp,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub owner: String,
    pub event: BattleEvent,
    /// Unit-event subject filter; global events require None.
    pub subject: Option<String>,
    pub action: Option<String>,
    /// Zero means unlimited; positive counts are consumed before dispatch.
    pub uses: u32,
    pub effects: Vec<Effect>,
    /// Expire before any listeners of this event. Global ticks apply to present
    /// owners; unit ticks apply only when the owner is the event subject.
    pub expires_on: Option<BattleEvent>,
    pub duration: u32,
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
fn validate_mechanics(s: &Scenario) -> Result<(), String> {
    let m = &s.mechanics;
    if let Some(capacity) = m.queue_capacity {
        if !(1..=32).contains(&capacity) || s.waves.len() != 1 {
            return Err(
                "queue mode requires capacity 1..32 and exactly one queue in waves[0]".into(),
            );
        }
    }
    if let Some(policy) = &m.settlement {
        policy.validate(s)?;
    }
    if m.assumptions.len() > 100 || m.assumptions.iter().any(|a| a.is_empty() || a.len() > 2000) {
        return Err("invalid assumptions ledger".into());
    }
    bounded(m.ally_damage_factor, 0.0, 1e6, "ally_damage_factor")?;
    if m.cycles.len() > 10000 {
        return Err("too many cycles".into());
    }
    for c in &m.cycles {
        bounded(*c, 0.000001, 1e9, "cycle AV")?;
    }
    if !m.cycles.is_empty() && (m.cycles.iter().sum::<f64>() - s.max_av).abs() > 1e-8 {
        return Err("cycle AV sum must equal max_av".into());
    }
    let units: Vec<_> = s.allies.iter().chain(s.waves.iter().flatten()).collect();
    for (id, natural) in &m.natural_weaknesses {
        if !units.iter().any(|u| &u.id == id) {
            return Err("unknown natural-weakness unit".into());
        }
        crate::weakness::validate_set(natural, "natural weakness", true)?;
    }
    for (id, attached) in &m.modifier_weaknesses {
        if !m.modifier_definitions.contains_key(id) {
            return Err(format!("unknown weakness modifier definition {id}"));
        }
        crate::weakness::validate_set(attached, "attached weakness", false)?;
    }
    if !m.modifier_weaknesses.is_empty()
        && units
            .iter()
            .any(|u| !m.natural_weaknesses.contains_key(&u.id))
    {
        return Err("weakness modifiers require explicit natural sets for every unit".into());
    }
    for (id, profile) in &m.element_profiles {
        if !units.iter().any(|u| &u.id == id) {
            return Err("unknown element-profile unit".into());
        }
        profile.validate()?;
    }
    for (id, definition) in &m.modifier_definitions {
        identifier(id, "modifier id")?;
        definition.validate()?;
    }
    for effects in units
        .iter()
        .flat_map(|u| u.actions.iter().map(|a| &a.effects))
        .chain(m.triggers.iter().map(|t| &t.effects))
    {
        for e in flattened_effects(effects)? {
            if let Effect::Conditional { condition, .. } = e {
                condition.validate_modifier_references(&m.modifier_definitions)?;
            }
            if matches!(e, Effect::ElementalDamage { .. })
                && units
                    .iter()
                    .any(|u| !m.element_profiles.contains_key(&u.id))
            {
                return Err(
                    "elemental damage requires explicit seven-element panels for every unit".into(),
                );
            }
            if let Effect::AddModifier { id, .. } | Effect::RemoveModifier { id, .. } = e {
                if !m.modifier_definitions.contains_key(id) {
                    return Err(format!("unknown modifier definition {id}"));
                }
            }
        }
    }
    for (id, values) in &m.dynamic_values {
        if !units.iter().any(|u| &u.id == id) {
            return Err("unknown dynamic-values unit".into());
        }
        for (key, value) in values {
            identifier(key, "dynamic key")?;
            if !value.is_finite() {
                return Err("nonfinite initial dynamic value".into());
            }
        }
    }
    for (id, t) in &m.toughness {
        if !units.iter().any(|u| &u.id == id) {
            return Err("unknown toughness unit".into());
        }
        bounded(*t, 0.0, 1e12, "toughness")?;
    }
    for t in &m.triggers {
        let owner = units
            .iter()
            .find(|u| u.id == t.owner)
            .ok_or("unknown trigger owner")?;
        if t.event.is_global() && t.subject.is_some() {
            return Err("global trigger event has no unit subject".into());
        }
        if t.subject
            .as_ref()
            .is_some_and(|id| !units.iter().any(|u| &u.id == id))
        {
            return Err("unknown trigger subject".into());
        }
        if t.expires_on.is_some() && t.duration == 0 {
            return Err("status duration must be positive".into());
        }
        let mut probe = (*owner).clone();
        probe.actions = vec![Action {
            id: "trigger_validation".into(),
            kind: ActionKind::Basic,
            sp_cost: 0,
            sp_gain: 0,
            energy_cost: 0.0,
            energy_gain: 0.0,
            effects: t.effects.clone(),
        }];
        validate_unit(
            &probe,
            &SkillPoints {
                initial: 0,
                maximum: 20,
            },
        )?;
    }
    Ok(())
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
