//! 实际游玩配置使用的统一属性键与数值单位。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 固定属性使用实际数值，百分比属性使用比例值。
pub type AttributeValues = BTreeMap<Attribute, f64>;

/// 角色、行迹、光锥和遗器共用的属性类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Attribute {
    /// 固定生命。
    #[serde(rename = "HP")]
    Hp,
    /// 固定攻击。
    #[serde(rename = "ATK")]
    Attack,
    /// 固定防御。
    #[serde(rename = "DEF")]
    Defense,
    /// 生命百分比。
    #[serde(rename = "HP_")]
    HpPercent,
    /// 攻击百分比。
    #[serde(rename = "ATK_")]
    AttackPercent,
    /// 防御百分比。
    #[serde(rename = "DEF_")]
    DefensePercent,
    /// 固定速度，保留小数。
    #[serde(rename = "SPD")]
    Speed,
    /// 速度百分比。
    #[serde(rename = "SPD_")]
    SpeedPercent,
    /// 暴击率。
    #[serde(rename = "CRIT Rate_")]
    CritRate,
    /// 暴击伤害。
    #[serde(rename = "CRIT DMG_")]
    CritDamage,
    /// 效果命中。
    #[serde(rename = "Effect Hit Rate_")]
    EffectHitRate,
    /// 效果抵抗。
    #[serde(rename = "Effect RES_")]
    EffectResistance,
    /// 击破特攻。
    #[serde(rename = "Break Effect_")]
    BreakEffect,
    /// 能量恢复效率。
    #[serde(rename = "Energy Regeneration Rate_")]
    EnergyRegenerationRate,
    /// 治疗量加成。
    #[serde(rename = "Outgoing Healing Boost_")]
    OutgoingHealingBoost,
    /// 受到治疗量加成。
    #[serde(rename = "Incoming Healing Boost_")]
    IncomingHealingBoost,
    /// 全属性伤害加成。
    #[serde(rename = "All DMG Boost_")]
    AllDamageBoost,
    /// 物理伤害加成。
    #[serde(rename = "Physical DMG Boost_")]
    PhysicalDamageBoost,
    /// 火伤害加成。
    #[serde(rename = "Fire DMG Boost_")]
    FireDamageBoost,
    /// 冰伤害加成。
    #[serde(rename = "Ice DMG Boost_")]
    IceDamageBoost,
    /// 雷伤害加成。
    #[serde(rename = "Lightning DMG Boost_")]
    LightningDamageBoost,
    /// 风伤害加成。
    #[serde(rename = "Wind DMG Boost_")]
    WindDamageBoost,
    /// 量子伤害加成。
    #[serde(rename = "Quantum DMG Boost_")]
    QuantumDamageBoost,
    /// 虚数伤害加成。
    #[serde(rename = "Imaginary DMG Boost_")]
    ImaginaryDamageBoost,
    /// 欢愉度。
    #[serde(rename = "Elation_")]
    Elation,
}

/// 某个来源提供的一项属性贡献。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeBonus {
    /// 属性键。
    pub attribute: Attribute,
    /// 固定值或比例值，由属性键确定单位。
    pub value: f64,
}
