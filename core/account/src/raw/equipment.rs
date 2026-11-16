//! 可分配装备实例与遗器部位。

use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{RelicMainStat, RelicSubStat};

/// 六个遗器部位，序列化为沿用 Python 模型的数字 1～6。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[repr(u8)]
pub enum RelicSlot {
    /// 头部，部位 1。
    Head = 1,
    /// 手部，部位 2。
    Hands = 2,
    /// 躯干，部位 3。
    Body = 3,
    /// 脚部，部位 4。
    Feet = 4,
    /// 位面球，部位 5。
    PlanarSphere = 5,
    /// 连结绳，部位 6。
    LinkRope = 6,
}

/// 无法映射到六个遗器部位的数字编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRelicSlot(pub u8);

impl fmt::Display for InvalidRelicSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "遗器部位必须为 1～6，收到 {}", self.0)
    }
}

impl Error for InvalidRelicSlot {}

impl TryFrom<u8> for RelicSlot {
    type Error = InvalidRelicSlot;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Head),
            2 => Ok(Self::Hands),
            3 => Ok(Self::Body),
            4 => Ok(Self::Feet),
            5 => Ok(Self::PlanarSphere),
            6 => Ok(Self::LinkRope),
            _ => Err(InvalidRelicSlot(value)),
        }
    }
}

impl From<RelicSlot> for u8 {
    fn from(value: RelicSlot) -> Self {
        value as u8
    }
}

/// 一件可独立分配的原始光锥。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConeInstance {
    /// 配置内唯一的光锥实例 ID；来源适配器负责分配实例命名空间。
    pub instance_id: String,
    /// 光锥种类 ID，同类的多个实例使用同一值。
    pub light_cone_id: String,
    /// 原始等级。
    pub level: u8,
    /// 原始突破阶段，当前资料取 0～6。
    pub promotion: u8,
    /// 叠影等级，当前资料取 1～5。
    pub superimposition: u8,
}

/// 一件原始遗器，保留实际品质、强化等级与当前副词条。
///
/// 主词条最终数值按品质、强化等级与资料快照查询。副词条保存当前最终值，
/// 百分比采用比例值，例如 `0.0324` 表示 3.24%；速度等固定属性保留小数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicInstance {
    /// 配置内唯一的遗器实例 ID。
    pub instance_id: String,
    /// 遗器套装 ID。
    pub set_id: String,
    /// 遗器部位。
    pub slot: RelicSlot,
    /// 实际星级；由对应版本的资料检查支持的品质范围。
    pub rarity: u8,
    /// 实际强化等级，允许未强化和未满强化。
    pub level: u8,
    /// 已区分固定数值与百分比的主词条键。
    pub mainstat: RelicMainStat,
    /// 当前已生效的副词条；未获得副词条时为空，来源预览词条另行保存。
    pub substats: BTreeMap<RelicSubStat, f64>,
}

impl RelicInstance {
    /// 遗器最多包含的互异副词条数量。
    pub const MAX_SUBSTATS: usize = 4;

    /// 按品质读取强化上限；当前支持二至五星遗器。
    pub fn max_level_for_rarity(rarity: u8) -> Option<u8> {
        (2..=5).contains(&rarity).then_some(rarity * 3)
    }
}
