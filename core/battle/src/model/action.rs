//! 动作、效果树与条件的输入定义。
use super::*;
use serde::{Deserialize, Serialize};

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

impl Target {
    /// 伤害效果允许的敌方选择器，由输入校验统一使用。
    pub(super) fn is_enemy(self) -> bool {
        matches!(
            self,
            Self::ContextTarget
                | Self::AbilityTarget
                | Self::EnemyLowestHp
                | Self::EnemyRandomTaunt
                | Self::EnemyRandomUniform
                | Self::AllEnemies
        )
    }
}
