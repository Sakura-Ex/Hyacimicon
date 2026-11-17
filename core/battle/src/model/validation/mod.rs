//! 单次输入校验与场景指纹；各对象的规则按职责维护。
use super::*;
use std::collections::BTreeSet;

mod effects;
mod mechanics;
mod unit;

use effects::{flattened_effects, validate_effects};
use mechanics::validate_mechanics;
use unit::{validate_healing_targets, validate_unit};

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
