//! 伤害、治疗与护盾的确定性数值计算。
//!
//! 调用方组装基础值、加成和暴击结果；本模块返回计算值与伤害乘区明细。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DamageInput {
    pub base_damage: f64,
    pub attacker_level: u16,
    pub defense: f64,
    pub defense_reduction: f64,
    pub defense_ignore: f64,
    pub resistance: f64,
    pub resistance_penetration: f64,
    /// Aggregated additive bonus ratio, e.g. 0.2 means +20%.
    pub damage_bonus: f64,
    pub vulnerability: f64,
    pub toughness_broken: bool,
    /// Aggregated damage-reduction ratio, not the retained multiplier.
    pub damage_reduction: f64,
    /// Already resolved independent multiplier; neutral is 1, not 0.
    pub independent_multiplier: f64,
    pub crit_damage: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DamageBreakdown {
    pub base_damage: f64,
    pub damage_bonus_multiplier: f64,
    pub independent_multiplier: f64,
    pub defense_multiplier: f64,
    pub resistance_multiplier: f64,
    pub toughness_multiplier: f64,
    pub vulnerability_multiplier: f64,
    pub critical_multiplier: f64,
    pub damage_reduction_multiplier: f64,
    pub critical: bool,
    pub amount: f64,
}

fn finite(name: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{name} must be finite"))
    }
}

fn at_least(name: &str, value: f64, lower: f64) -> Result<(), String> {
    finite(name, value)?;
    if value < lower {
        return Err(format!("{name} must be >= {lower}"));
    }
    Ok(())
}

fn ratio(name: &str, value: f64) -> Result<(), String> {
    at_least(name, value, 0.0)?;
    if value > 1.0 {
        return Err(format!("{name} must be <= 1"));
    }
    Ok(())
}

/// 校验输入后按固定顺序计算伤害乘区，保留浮点结果。
pub fn damage(input: &DamageInput, critical: bool) -> Result<DamageBreakdown, String> {
    at_least("base_damage", input.base_damage, 0.0)?;
    if input.attacker_level == 0 {
        return Err("attacker_level must be positive".into());
    }
    at_least("defense", input.defense, 0.0)?;
    at_least("defense_reduction", input.defense_reduction, 0.0)?;
    at_least("defense_ignore", input.defense_ignore, 0.0)?;
    finite("resistance", input.resistance)?;
    finite("resistance_penetration", input.resistance_penetration)?;
    at_least("damage_bonus", input.damage_bonus, -1.0)?;
    at_least("vulnerability", input.vulnerability, -1.0)?;
    ratio("damage_reduction", input.damage_reduction)?;
    at_least("independent_multiplier", input.independent_multiplier, 0.0)?;
    at_least("crit_damage", input.crit_damage, 0.0)?;

    let level_term = f64::from(input.attacker_level) * 10.0 + 200.0;
    let defense_penetration = finite(
        "combined defense penetration",
        input.defense_reduction + input.defense_ignore,
    )?;
    let defense_remaining = (1.0 - defense_penetration).max(0.0);
    let defense_multiplier = level_term
        / finite(
            "defense denominator",
            input.defense * defense_remaining + level_term,
        )?;
    let effective_resistance = finite(
        "effective resistance",
        input.resistance - input.resistance_penetration,
    )?;
    let resistance_multiplier = 1.0 - effective_resistance.clamp(-1.0, 0.9);
    let damage_bonus_multiplier = finite("damage bonus multiplier", 1.0 + input.damage_bonus)?;
    let vulnerability_multiplier = finite("vulnerability multiplier", 1.0 + input.vulnerability)?;
    let toughness_multiplier = if input.toughness_broken { 1.0 } else { 0.9 };
    let critical_multiplier = if critical {
        finite("critical multiplier", 1.0 + input.crit_damage)?
    } else {
        1.0
    };
    let damage_reduction_multiplier = 1.0 - input.damage_reduction;

    // 当前接口中的最终伤害与虚弱乘区取 1。
    let mut amount = input.base_damage;
    for factor in [
        damage_bonus_multiplier,
        input.independent_multiplier,
        defense_multiplier,
        resistance_multiplier,
        toughness_multiplier,
        vulnerability_multiplier,
        1.0,
        critical_multiplier,
        1.0,
        damage_reduction_multiplier,
    ] {
        amount = finite("damage product", amount * factor)?;
    }
    Ok(DamageBreakdown {
        base_damage: input.base_damage,
        damage_bonus_multiplier,
        independent_multiplier: input.independent_multiplier,
        defense_multiplier,
        resistance_multiplier,
        toughness_multiplier,
        vulnerability_multiplier,
        critical_multiplier,
        damage_reduction_multiplier,
        critical,
        amount,
    })
}

/// 对已组装的治疗基础值应用治疗加成与受治疗加成。
/// 两项加成各自至少为 -1，合计后的治疗乘数须非负。
pub fn heal(base_amount: f64, outgoing_boost: f64, incoming_boost: f64) -> Result<f64, String> {
    at_least("base_amount", base_amount, 0.0)?;
    at_least("outgoing_boost", outgoing_boost, -1.0)?;
    at_least("incoming_boost", incoming_boost, -1.0)?;
    let multiplier = finite("healing multiplier", 1.0 + outgoing_boost + incoming_boost)?;
    at_least("healing multiplier", multiplier, 0.0)?;
    finite("healing amount", base_amount * multiplier)
}

/// 对已组装的护盾基础值应用护盾加成。
pub fn shield(base_amount: f64, outgoing_boost: f64) -> Result<f64, String> {
    at_least("base_amount", base_amount, 0.0)?;
    at_least("outgoing_boost", outgoing_boost, -1.0)?;
    let multiplier = finite("shield multiplier", 1.0 + outgoing_boost)?;
    finite("shield amount", base_amount * multiplier)
}
