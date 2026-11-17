//! 汇总静态属性并计算基础值与加成值。

use std::collections::BTreeMap;

use crate::{
    RelicMainStat, RelicSubStat,
    reference::{CharacterData, Element, LightConeData},
};

use super::{
    BASE_CRIT_DAMAGE, CharacterPanel, PanelStat,
    attributes::{Attribute, AttributeValues},
    conversion::ConversionError,
};

/// 将一个来源的属性加入合计。
pub(super) fn add_values(total: &mut AttributeValues, values: &AttributeValues) {
    for (key, value) in values {
        *total.entry(*key).or_default() += value;
    }
}

/// 未出现的加成属性采用加法单位元零。
fn bonus(values: &AttributeValues, key: Attribute) -> f64 {
    values.get(&key).copied().unwrap_or(0.0)
}

/// 计算并检查最终面板，拒绝非有限分量及总值。
pub(super) fn calculate(
    character: &CharacterData,
    cone: &LightConeData,
    values: &AttributeValues,
) -> Result<CharacterPanel, ConversionError> {
    let scaled = |base, flat, percent| PanelStat {
        base,
        added: base * bonus(values, percent) + bonus(values, flat),
    };
    let elemental_damage_boost = [
        (Element::Physical, Attribute::PhysicalDamageBoost),
        (Element::Fire, Attribute::FireDamageBoost),
        (Element::Ice, Attribute::IceDamageBoost),
        (Element::Lightning, Attribute::LightningDamageBoost),
        (Element::Wind, Attribute::WindDamageBoost),
        (Element::Quantum, Attribute::QuantumDamageBoost),
        (Element::Imaginary, Attribute::ImaginaryDamageBoost),
    ]
    .into_iter()
    .filter_map(|(element, key)| {
        let value = bonus(values, key);
        (value != 0.0).then_some((element, value))
    })
    .collect::<BTreeMap<_, _>>();
    let panel = CharacterPanel {
        hp: scaled(
            character.base.hp + cone.base.hp,
            Attribute::Hp,
            Attribute::HpPercent,
        ),
        attack: scaled(
            character.base.attack + cone.base.attack,
            Attribute::Attack,
            Attribute::AttackPercent,
        ),
        defense: scaled(
            character.base.defense + cone.base.defense,
            Attribute::Defense,
            Attribute::DefensePercent,
        ),
        speed: scaled(
            character.base.speed + bonus(values, Attribute::BaseSpeed),
            Attribute::Speed,
            Attribute::SpeedPercent,
        ),
        aggro: f64::from(character.base.aggro),
        crit_rate: character.base.crit_rate() + bonus(values, Attribute::CritRate),
        crit_damage: BASE_CRIT_DAMAGE + bonus(values, Attribute::CritDamage),
        effect_hit_rate: bonus(values, Attribute::EffectHitRate),
        effect_resistance: bonus(values, Attribute::EffectResistance),
        break_effect: bonus(values, Attribute::BreakEffect),
        energy_regeneration_rate: 1.0 + bonus(values, Attribute::EnergyRegenerationRate),
        outgoing_healing_boost: bonus(values, Attribute::OutgoingHealingBoost),
        incoming_healing_boost: bonus(values, Attribute::IncomingHealingBoost),
        all_damage_boost: bonus(values, Attribute::AllDamageBoost),
        elemental_damage_boost,
        elation: bonus(values, Attribute::Elation),
    };
    let finite_scaled = [&panel.hp, &panel.attack, &panel.defense, &panel.speed]
        .into_iter()
        .all(|stat| stat.base.is_finite() && stat.added.is_finite() && stat.total().is_finite());
    let finite_other = [
        panel.aggro,
        panel.crit_rate,
        panel.crit_damage,
        panel.effect_hit_rate,
        panel.effect_resistance,
        panel.break_effect,
        panel.energy_regeneration_rate,
        panel.outgoing_healing_boost,
        panel.incoming_healing_boost,
        panel.all_damage_boost,
        panel.elation,
    ]
    .into_iter()
    .chain(panel.elemental_damage_boost.values().copied())
    .all(f64::is_finite);
    if !finite_scaled || !finite_other {
        return Err(ConversionError(format!(
            "角色 {} 的静态面板包含非有限数值",
            character.id
        )));
    }
    Ok(panel)
}

impl From<RelicMainStat> for Attribute {
    fn from(value: RelicMainStat) -> Self {
        match value {
            RelicMainStat::Hp => Self::Hp,
            RelicMainStat::Atk => Self::Attack,
            RelicMainStat::HpPercent => Self::HpPercent,
            RelicMainStat::AtkPercent => Self::AttackPercent,
            RelicMainStat::DefPercent => Self::DefensePercent,
            RelicMainStat::Spd => Self::Speed,
            RelicMainStat::CritRate => Self::CritRate,
            RelicMainStat::CritDmg => Self::CritDamage,
            RelicMainStat::EffectHitRate => Self::EffectHitRate,
            RelicMainStat::OutgoingHealingBoost => Self::OutgoingHealingBoost,
            RelicMainStat::BreakEffect => Self::BreakEffect,
            RelicMainStat::EnergyRegenerationRate => Self::EnergyRegenerationRate,
            RelicMainStat::PhysicalDmgBoost => Self::PhysicalDamageBoost,
            RelicMainStat::FireDmgBoost => Self::FireDamageBoost,
            RelicMainStat::IceDmgBoost => Self::IceDamageBoost,
            RelicMainStat::LightningDmgBoost => Self::LightningDamageBoost,
            RelicMainStat::WindDmgBoost => Self::WindDamageBoost,
            RelicMainStat::QuantumDmgBoost => Self::QuantumDamageBoost,
            RelicMainStat::ImaginaryDmgBoost => Self::ImaginaryDamageBoost,
        }
    }
}

impl From<RelicSubStat> for Attribute {
    fn from(value: RelicSubStat) -> Self {
        match value {
            RelicSubStat::Hp => Self::Hp,
            RelicSubStat::Atk => Self::Attack,
            RelicSubStat::Def => Self::Defense,
            RelicSubStat::HpPercent => Self::HpPercent,
            RelicSubStat::AtkPercent => Self::AttackPercent,
            RelicSubStat::DefPercent => Self::DefensePercent,
            RelicSubStat::Spd => Self::Speed,
            RelicSubStat::CritRate => Self::CritRate,
            RelicSubStat::CritDmg => Self::CritDamage,
            RelicSubStat::EffectHitRate => Self::EffectHitRate,
            RelicSubStat::EffectRes => Self::EffectResistance,
            RelicSubStat::BreakEffect => Self::BreakEffect,
        }
    }
}
