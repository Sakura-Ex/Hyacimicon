use crate::{ResourceDelta, Resources, SettlementRules};
use battle_core::model::Outcome;

pub(crate) fn identifier(text: &str, name: &str) -> Result<(), String> {
    if text.is_empty() || text.len() > 200 || text.trim() != text {
        return Err(format!("invalid {name}"));
    }
    Ok(())
}

pub(crate) fn validate_resources(value: &Resources) -> Result<(), String> {
    if !value.max_hp.is_finite()
        || value.max_hp <= 0.0
        || value.max_hp > 1e12
        || !value.hp.is_finite()
        || value.hp < 0.0
        || value.hp > value.max_hp
    {
        return Err("HP must be finite: 0 <= hp <= max_hp <= 1e12 and max_hp > 0".into());
    }
    Ok(())
}

/// Validate explicit synthetic rules. None means unknown, including a missing
/// outcome branch; it is never converted to a zero delta.
pub fn validate_rules(rules: &SettlementRules) -> Result<(), String> {
    if rules.ruleset != "synthetic_explicit_v1" {
        return Err(
            "unsupported settlement ruleset; only synthetic_explicit_v1 is implemented".into(),
        );
    }
    identifier(&rules.rule_id, "settlement rule_id")?;
    identifier(&rules.source, "settlement source")?;
    for delta in [&rules.victory, &rules.defeat, &rules.draw]
        .into_iter()
        .flatten()
    {
        if !delta.hp.is_finite() || delta.hp.abs() > 1e12 {
            return Err("settlement HP delta must be finite in [-1e12, 1e12]".into());
        }
        let bound = i64::from(u32::MAX);
        if delta.gold < -bound
            || delta.gold > bound
            || delta.crystals < -bound
            || delta.crystals > bound
        {
            return Err("settlement money delta exceeds exact u32 balance range".into());
        }
    }
    Ok(())
}

pub(crate) fn applicable(rules: &SettlementRules, outcome: Outcome) -> Option<&ResourceDelta> {
    match outcome {
        Outcome::Victory => rules.victory.as_ref(),
        Outcome::Defeat => rules.defeat.as_ref(),
        Outcome::Draw => rules.draw.as_ref(),
        Outcome::Incomplete => None,
    }
}

pub(crate) fn apply(
    resources: &Resources,
    delta: &ResourceDelta,
) -> Result<(Resources, ResourceDelta), String> {
    let raw_hp = resources.hp + delta.hp;
    if !raw_hp.is_finite() {
        return Err("nonfinite HP settlement".into());
    }
    // This is the explicit synthetic profile: HP clamps, money rejects overflow
    // and underflow, and max_hp is unchanged. No live-game rule is inferred.
    let hp = raw_hp.clamp(0.0, resources.max_hp);
    let gold = u32::try_from(i64::from(resources.gold) + delta.gold)
        .map_err(|_| "gold settlement outside u32 balance range".to_string())?;
    let crystals = u32::try_from(i64::from(resources.crystals) + delta.crystals)
        .map_err(|_| "crystal settlement outside u32 balance range".to_string())?;
    let after = Resources {
        hp,
        max_hp: resources.max_hp,
        gold,
        crystals,
    };
    let actual = ResourceDelta {
        hp: after.hp - resources.hp,
        gold: i64::from(after.gold) - i64::from(resources.gold),
        crystals: i64::from(after.crystals) - i64::from(resources.crystals),
    };
    Ok((after, actual))
}
