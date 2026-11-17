//! 校验单位面板、动作资源与可共存的治疗对象。
use super::*;

pub(super) fn validate_unit(unit: &Unit, sp: &SkillPoints) -> Result<(), String> {
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
        validate_effects(&a.effects)?;
    }
    if !fallback {
        return Err("unit requires a free non-ultimate fallback action".into());
    }
    Ok(())
}

/// Validate every potential healing pair on one coexisting team. Target choice
/// may change with HP and deaths, so initial HP alone cannot narrow recipients.
pub(super) fn validate_healing_targets(team: &[Unit]) -> Result<(), String> {
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
