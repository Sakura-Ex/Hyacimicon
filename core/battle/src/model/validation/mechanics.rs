//! 校验显式机制参数、单位引用和监听器效果。
use super::*;

pub(super) fn validate_mechanics(s: &Scenario) -> Result<(), String> {
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
        if !units.iter().any(|u| u.id == t.owner) {
            return Err("unknown trigger owner".into());
        }
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
        validate_effects(&t.effects)?;
    }
    Ok(())
}
