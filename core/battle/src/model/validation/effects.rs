//! 校验完整效果树、条件与伤害目标选择器。
use super::*;

impl Condition {
    pub(super) fn validate_modifier_references(
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
pub(super) fn flattened_effects(effects: &[Effect]) -> Result<Vec<&Effect>, String> {
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

pub(super) fn validate_effects(effects: &[Effect]) -> Result<(), String> {
    if effects.is_empty() || effects.len() > 32 {
        return Err("effects must contain 1..32 entries".into());
    }
    for e in flattened_effects(effects)? {
        match e {
            Effect::ElementalDamage {
                ratio,
                flat,
                target,
                ..
            } => {
                ratio.validate()?;
                flat.validate()?;
                if !target.is_enemy() {
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
                if !target.is_enemy() {
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
                    return Err("extra turn selects at entry; other requests require action".into());
                }
            }
            Effect::AdvanceNext { fraction, .. } | Effect::Advance { fraction, .. } => {
                bounded(*fraction, -1.0, 1.0, "advance fraction")?
            }
        }
    }
    Ok(())
}
