//! 遗器属性键，序列化名称沿用现有账号生成器的统一口径。
//!
//! 百分比属性键以 `_` 结尾，其数值使用比例；固定属性使用实际数值。

use serde::{Deserialize, Serialize};

/// 遗器主词条类型；主词条的最终数值由资料快照提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RelicMainStat {
    /// 固定生命值。
    #[serde(rename = "HP")]
    Hp,
    /// 固定攻击力。
    #[serde(rename = "ATK")]
    Atk,
    /// 生命值百分比。
    #[serde(rename = "HP_")]
    HpPercent,
    /// 攻击力百分比。
    #[serde(rename = "ATK_")]
    AtkPercent,
    /// 防御力百分比。
    #[serde(rename = "DEF_")]
    DefPercent,
    /// 速度。
    #[serde(rename = "SPD")]
    Spd,
    /// 暴击率。
    #[serde(rename = "CRIT Rate_")]
    CritRate,
    /// 暴击伤害。
    #[serde(rename = "CRIT DMG_")]
    CritDmg,
    /// 效果命中。
    #[serde(rename = "Effect Hit Rate_")]
    EffectHitRate,
    /// 治疗量加成。
    #[serde(rename = "Outgoing Healing Boost_")]
    OutgoingHealingBoost,
    /// 击破特攻。
    #[serde(rename = "Break Effect_")]
    BreakEffect,
    /// 能量恢复效率。
    #[serde(rename = "Energy Regeneration Rate_")]
    EnergyRegenerationRate,
    /// 物理属性伤害提高。
    #[serde(rename = "Physical DMG Boost_")]
    PhysicalDmgBoost,
    /// 火属性伤害提高。
    #[serde(rename = "Fire DMG Boost_")]
    FireDmgBoost,
    /// 冰属性伤害提高。
    #[serde(rename = "Ice DMG Boost_")]
    IceDmgBoost,
    /// 雷属性伤害提高。
    #[serde(rename = "Lightning DMG Boost_")]
    LightningDmgBoost,
    /// 风属性伤害提高。
    #[serde(rename = "Wind DMG Boost_")]
    WindDmgBoost,
    /// 量子属性伤害提高。
    #[serde(rename = "Quantum DMG Boost_")]
    QuantumDmgBoost,
    /// 虚数属性伤害提高。
    #[serde(rename = "Imaginary DMG Boost_")]
    ImaginaryDmgBoost,
}

/// 遗器副词条类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RelicSubStat {
    /// 固定生命值。
    #[serde(rename = "HP")]
    Hp,
    /// 固定攻击力。
    #[serde(rename = "ATK")]
    Atk,
    /// 固定防御力。
    #[serde(rename = "DEF")]
    Def,
    /// 生命值百分比。
    #[serde(rename = "HP_")]
    HpPercent,
    /// 攻击力百分比。
    #[serde(rename = "ATK_")]
    AtkPercent,
    /// 防御力百分比。
    #[serde(rename = "DEF_")]
    DefPercent,
    /// 速度，保留小数精度。
    #[serde(rename = "SPD")]
    Spd,
    /// 暴击率。
    #[serde(rename = "CRIT Rate_")]
    CritRate,
    /// 暴击伤害。
    #[serde(rename = "CRIT DMG_")]
    CritDmg,
    /// 效果命中。
    #[serde(rename = "Effect Hit Rate_")]
    EffectHitRate,
    /// 效果抵抗。
    #[serde(rename = "Effect RES_")]
    EffectRes,
    /// 击破特攻。
    #[serde(rename = "Break Effect_")]
    BreakEffect,
}

impl RelicMainStat {
    /// 检查主词条是否适用于指定部位。
    pub fn supports_slot(self, slot: crate::RelicSlot) -> bool {
        use crate::RelicMainStat as Stat;
        use crate::RelicSlot;
        let stat = self;
        match slot {
            RelicSlot::Head => stat == Stat::Hp,
            RelicSlot::Hands => stat == Stat::Atk,
            RelicSlot::Body => matches!(
                stat,
                Stat::HpPercent
                    | Stat::AtkPercent
                    | Stat::DefPercent
                    | Stat::CritRate
                    | Stat::CritDmg
                    | Stat::EffectHitRate
                    | Stat::OutgoingHealingBoost
            ),
            RelicSlot::Feet => matches!(
                stat,
                Stat::HpPercent | Stat::AtkPercent | Stat::DefPercent | Stat::Spd
            ),
            RelicSlot::PlanarSphere => matches!(
                stat,
                Stat::HpPercent
                    | Stat::AtkPercent
                    | Stat::DefPercent
                    | Stat::PhysicalDmgBoost
                    | Stat::FireDmgBoost
                    | Stat::IceDmgBoost
                    | Stat::LightningDmgBoost
                    | Stat::WindDmgBoost
                    | Stat::QuantumDmgBoost
                    | Stat::ImaginaryDmgBoost
            ),
            RelicSlot::LinkRope => matches!(
                stat,
                Stat::HpPercent
                    | Stat::AtkPercent
                    | Stat::DefPercent
                    | Stat::BreakEffect
                    | Stat::EnergyRegenerationRate
            ),
        }
    }
}
