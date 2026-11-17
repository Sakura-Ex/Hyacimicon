//! 直接伤害、治疗和护盾的确定性公式，参考 HSR_Nous 实现。
//!
//! Upstream commit: f8935ddb407b297732fbd3c624d55897e3e648b9.
//! Source: src/hsr_nous/sim_schema/rulebook.yaml, lines 234-238, 262-266,
//! 279-375. SHA-256: d37c1b0a9dbe3819cd210e02bef96db9598486ab9b8554bad90602ca136277fe.
//! The direct-damage subset takes an already assembled base and bonus pool.
//! Upstream final-damage and weaken zones are neutral (1) in this interface.
//! Critical outcome is provided by the caller; this module consumes no RNG.
//! Healing/shielding return raw amounts; application, caps, duration, stacking,
//! target selection, costs, and resource gains belong to the simulation layer.
//!
//! MIT License
//!
//! Copyright (c) 2026 The HSR_Nous Authors
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

#[derive(Debug, Clone)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 一次直接伤害所需的已组装乘区参数。
pub struct DamageInput {
    /// 已经合成的单次基础伤害。
    pub base_damage: f64,
    /// 攻击方等级。
    pub attacker_level: u16,
    /// 静态防御力或本击目标防御。
    pub defense: f64,
    /// 攻击方携带的有效减防比例。
    pub defense_reduction: f64,
    /// 攻击方无视防御的比例。
    pub defense_ignore: f64,
    /// 目标抗性或按元素关联的完整抗性。
    pub resistance: f64,
    /// 攻击方的抗性穿透。
    pub resistance_penetration: f64,
    /// Aggregated additive bonus ratio, e.g. 0.2 means +20%.
    pub damage_bonus: f64,
    /// 目标易伤比例。
    pub vulnerability: f64,
    /// 目标是否处于韧性击破状态。
    pub toughness_broken: bool,
    /// Aggregated damage-reduction ratio, not the retained multiplier.
    pub damage_reduction: f64,
    /// Already resolved independent multiplier; neutral is 1, not 0.
    pub independent_multiplier: f64,
    /// 暴击伤害加成，暴击倍率为一加此值。
    pub crit_damage: f64,
}

/// 返回有限计算结果，遇到 NaN 或无穷值时附带字段名报错。
fn finite(name: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{name} must be finite"))
    }
}

/// 按约定顺序应用已组装的直接伤害乘区，返回有限的伤害量。
///
/// 参数的静态范围由组装方保证；暴击结果由调用方采样。沿用参考实现的
/// original multiplication order. No rounding to display integers is performed.
pub fn damage(input: &DamageInput, critical: bool) -> Result<f64, String> {
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

    // final_dmg_multi and weaken_multi are deliberately neutral in this subset.
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
    Ok(amount)
}

/// Upstream heal formula after the caller assembles its ATK/HP/flat base.
/// Negative incoming boost is supported down to -1 (e.g. healing received down).
/// The combined multiplier must remain nonnegative; no silent lower clamp.
pub fn heal(base_amount: f64, outgoing_boost: f64, incoming_boost: f64) -> Result<f64, String> {
    let multiplier = finite("healing multiplier", 1.0 + outgoing_boost + incoming_boost)?;
    if multiplier < 0.0 {
        return Err("negative healing multiplier".into());
    }
    finite("healing amount", base_amount * multiplier)
}

/// Upstream shield formula after the caller assembles its DEF/HP/ATK/flat base.
pub fn shield(base_amount: f64, outgoing_boost: f64) -> Result<f64, String> {
    let multiplier = finite("shield multiplier", 1.0 + outgoing_boost)?;
    finite("shield amount", base_amount * multiplier)
}
