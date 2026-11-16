//! 完整转换生成的统一游玩配置、静态面板及共用规则。
//!
//! 面板和技能等级均为生效值，等级与进阶采用共用规则。试用与映射装备
//! 随对应角色保存；局内星级、站位和投资在战斗组装时加入。

pub mod attributes;
pub mod conversion;
mod panel;
mod selection;
pub mod traces;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    RelicInstance,
    reference::{Element, RelicGroupTemplate, TrialRelics},
};

/// 实际游玩配置的序列化格式版本。
pub const PLAYABLE_ACCOUNT_SCHEMA_VERSION: u32 = 2;

/// 玩法中角色与光锥的统一等级。
pub const PLAYABLE_LEVEL: u8 = 80;
/// 玩法中角色与光锥的统一进阶。
pub const PLAYABLE_PROMOTION: u8 = 6;
/// 试用与默认映射光锥的统一叠影。
pub const TEMPLATE_LIGHT_CONE_SUPERIMPOSITION: u8 = 1;
/// 遗器整圈映射所要求的强化等级。
pub const RELIC_MAPPING_LEVEL: u8 = 15;
/// 无特殊基础暴击率时使用的数值。
pub const BASE_CRIT_RATE: f64 = 0.05;
/// 角色统一基础暴击伤害。
pub const BASE_CRIT_DAMAGE: f64 = 0.5;

/// 一次配置转换使用的账号资源模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationMode {
    /// 持有角色使用本体，缺席角色补齐试用。
    OwnedWithTrials,
    /// 按挑战设置对全部角色使用试用模板。
    TrialsOnly,
}

/// 角色配置来源，保留本体与试用的区别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CharacterOrigin {
    /// 本体配置，关联转换前的角色 ID。
    Owned {
        /// 原始角色 ID，显式别名映射后可能与目标 ID 不同。
        raw_character_id: String,
    },
    /// 未持有或全试用模式采用的模板。
    Trial,
}

/// 装备使用原始库存实例，或来自某个角色专用模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EquipmentOrigin {
    /// 原始账号的库存实例。
    Owned {
        /// 原始装备实例 ID。
        instance_id: String,
    },
    /// 随角色使用的试用装备。
    Trial,
    /// 规则替换后的装备。
    Mapped,
}

/// 使用统一等级与进阶的生效光锥。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableLightCone {
    /// 光锥种类 ID。
    pub light_cone_id: String,
    /// 生效叠影。
    pub superimposition: u8,
    /// 库存实例或角色专用模板来源。
    pub origin: EquipmentOrigin,
}

/// 一圈保留原装，或整圈使用映射合计。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayableRelicGroup {
    /// 使用确认持有且全部达到映射等级的实际实例。
    Owned {
        /// 此圈实际使用的实例 ID。
        instance_ids: Vec<String>,
    },
    /// 使用文档给出的整圈配置。
    Mapped {
        /// 整圈套装和主副词条合计。
        configuration: RelicGroupTemplate,
    },
}

/// 文档允许表达的两类遗器配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayableRelics {
    /// 本体内外圈分别保留或映射。
    Partitioned {
        /// 外圈。
        outer: PlayableRelicGroup,
        /// 内圈。
        planar: PlayableRelicGroup,
    },
    /// 试用表提供的六件合计，直接作为完整配置保存。
    TrialAggregate {
        /// 两套装 ID 及六件主副词条合计。
        configuration: TrialRelics,
    },
}

/// 星魂加级后的技能等级与节点解锁状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableTraces {
    /// 技能 ID 到包含星魂附加等级的最终技能等级。
    pub skill_levels: BTreeMap<String, u8>,
    /// 一次性行迹节点的解锁状态。
    pub node_unlocks: BTreeMap<String, bool>,
}

/// 一项属性的基础值及静态加成。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelStat {
    /// 角色和光锥提供的基础值。
    pub base: f64,
    /// 固定加成与百分比加成折算后的增加值。
    pub added: f64,
}

impl PanelStat {
    /// 读取基础值与加成之和。
    pub fn total(&self) -> f64 {
        self.base + self.added
    }
}

/// 静态来源已计入的角色面板，供后续加入局内因素。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterPanel {
    /// 面板生命。
    pub hp: PanelStat,
    /// 面板攻击。
    pub attack: PanelStat,
    /// 面板防御。
    pub defense: PanelStat,
    /// 面板速度。
    pub speed: PanelStat,
    /// 嘲讽值。
    pub aggro: f64,
    /// 暴击率。
    pub crit_rate: f64,
    /// 暴击伤害。
    pub crit_damage: f64,
    /// 效果命中。
    pub effect_hit_rate: f64,
    /// 效果抵抗。
    pub effect_resistance: f64,
    /// 击破特攻。
    pub break_effect: f64,
    /// 能量恢复效率，以 1 为基础值。
    pub energy_regeneration_rate: f64,
    /// 治疗量加成。
    pub outgoing_healing_boost: f64,
    /// 受到治疗量加成。
    pub incoming_healing_boost: f64,
    /// 全属性伤害加成。
    pub all_damage_boost: f64,
    /// 各属性伤害加成。
    pub elemental_damage_boost: BTreeMap<Element, f64>,
    /// 欢愉度。
    pub elation: f64,
}

/// 已完成模板展开、映射和面板计算的一种角色配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableCharacter {
    /// 玩法资料使用的角色 ID。
    pub character_id: String,
    /// 本体、试用或玩法专属来源。
    pub origin: CharacterOrigin,
    /// 生效星魂数。
    pub eidolons: u8,
    /// 常规角色固定使用的能力版本；玩法专属角色无常规版本时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ability_version: Option<u32>,
    /// 基础行迹与星魂加级后的技能。
    pub traces: PlayableTraces,
    /// 生效光锥。
    pub light_cone: PlayableLightCone,
    /// 保留原装或采用模板的遗器配置。
    pub relics: PlayableRelics,
    /// 已完成的静态面板。
    pub panel: CharacterPanel,
}

/// 供策略与战斗组装共用的完整实际游玩配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableAccount {
    /// 实际游玩配置格式版本，使用 `PLAYABLE_ACCOUNT_SCHEMA_VERSION`。
    pub schema_version: u32,
    /// 生效配置标识。
    pub config_id: String,
    /// 对应原始账号配置标识。
    pub raw_config_id: String,
    /// 游戏版本。
    pub game_version: String,
    /// 玩法赛季。
    pub season: String,
    /// 数据快照。
    pub data_snapshot: String,
    /// 转换规则版本。
    pub rules_version: String,
    /// 本体加试用，或全试用模式。
    pub mode: ConfigurationMode,
    /// 玩法角色范围内的生效配置。
    pub characters: Vec<PlayableCharacter>,
    /// 使用统一等级与进阶的持有光锥库存。
    pub owned_light_cones: Vec<PlayableLightCone>,
    /// 确认持有的原始遗器库存。
    pub owned_relics: Vec<RelicInstance>,
}
