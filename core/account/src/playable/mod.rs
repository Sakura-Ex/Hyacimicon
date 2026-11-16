//! 实际游玩配置的目标结构；所需数据齐全后由后续转换器构造。
//!
//! 等级、面板和技能等级均为生效值。原始账号保持独立，试用与映射装备
//! 随对应角色保存；局内星级、站位和投资在战斗组装时加入。

pub mod attributes;
pub mod planning;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    LightConeInstance, RelicInstance, TraceConfig,
    reference::{Element, EnergyCapacity, RelicGroupTemplate, TrialRelics},
};

use self::attributes::AttributeValues;

/// 实际游玩配置的序列化格式版本。
pub const PLAYABLE_ACCOUNT_SCHEMA_VERSION: u32 = 1;

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
    Trial {
        /// 当前快照中的试用模板条目 ID。
        template_id: String,
    },
    /// 玩法专属角色采用的模板。
    GameplayExclusive {
        /// 玩法专属角色的试用模板条目 ID。
        template_id: String,
    },
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
    Trial {
        /// 来源模板条目 ID。
        template_id: String,
    },
    /// 规则替换后的装备。
    Mapped {
        /// 来源映射模板条目 ID。
        template_id: String,
    },
}

/// 已确定等级、突破和叠影的生效光锥。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableLightCone {
    /// 光锥种类 ID。
    pub light_cone_id: String,
    /// 生效等级。
    pub level: u8,
    /// 生效突破。
    pub promotion: u8,
    /// 生效叠影。
    pub superimposition: u8,
    /// 库存实例或角色专用模板来源。
    pub origin: EquipmentOrigin,
}

/// 一圈保留原装，或整圈使用映射合计。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayableRelicGroup {
    /// 使用确认持有的实际实例，允许空装和缺件。
    Owned {
        /// 此圈实际使用的实例 ID。
        instance_ids: Vec<String>,
    },
    /// 使用文档给出的整圈配置。
    Mapped {
        /// 映射模板条目 ID。
        template_id: String,
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
        /// 试用模板条目 ID。
        template_id: String,
        /// 两套装 ID 及六件主副词条合计。
        configuration: TrialRelics,
    },
}

/// 同时保存生效基础行迹和星魂计算后的技能等级。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableTraces {
    /// 映射或试用展开后的基础等级、真实节点解锁状态。
    pub base: TraceConfig,
    /// 技能 ID 到包含星魂附加等级的最终技能等级。
    pub skill_levels: BTreeMap<String, u8>,
}

/// 静态来源已计入的角色面板，供后续加入局内因素。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterPanel {
    /// 面板生命。
    pub hp: f64,
    /// 面板攻击。
    pub attack: f64,
    /// 面板防御。
    pub defense: f64,
    /// 面板速度。
    pub speed: f64,
    /// 嘲讽值，资料尚未提供时为空；依赖该值的战斗组装需补齐资料。
    pub aggro: Option<f64>,
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
    /// 各属性伤害加成。
    pub elemental_damage_boost: BTreeMap<Element, f64>,
    /// 欢愉度。
    pub elation: f64,
    /// 文档或后续资料确认的能量、专属资源容量。
    pub energy: EnergyCapacity,
}

/// 需由机制模块解释的效果引用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MechanismReference {
    /// 角色与能力版本。
    Character {
        /// 角色 ID。
        character_id: String,
        /// 能力版本。
        ability_version: u32,
    },
    /// 星魂机制。
    Eidolon {
        /// 角色 ID。
        character_id: String,
        /// 生效星魂数。
        count: u8,
    },
    /// 行迹机制。
    Trace {
        /// 已解锁节点 ID。
        node_id: String,
    },
    /// 光锥效果。
    LightCone {
        /// 光锥 ID。
        light_cone_id: String,
        /// 叠影。
        superimposition: u8,
    },
    /// 套装效果。
    RelicSet {
        /// 套装 ID。
        set_id: String,
        /// 实际件数。
        pieces: u8,
    },
}

/// 一个静态属性来源的贡献，用于核对面板。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelContribution {
    /// 来源类型与对应 ID，例如角色、行迹、光锥或遗器。
    pub source: String,
    /// 该来源贡献的固定值或百分比。
    pub attributes: AttributeValues,
}

/// 已完成模板展开、映射和面板计算的一种角色配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayableCharacter {
    /// 玩法资料使用的角色 ID。
    pub character_id: String,
    /// 本体、试用或玩法专属来源。
    pub origin: CharacterOrigin,
    /// 生效等级。
    pub level: u8,
    /// 生效突破。
    pub promotion: u8,
    /// 生效星魂数。
    pub eidolons: u8,
    /// 玩法固定使用的能力版本。
    pub ability_version: u32,
    /// 基础行迹与星魂加级后的技能。
    pub traces: PlayableTraces,
    /// 生效光锥，规则允许空装时可为空。
    pub light_cone: Option<PlayableLightCone>,
    /// 保留原装或采用模板的遗器配置。
    pub relics: PlayableRelics,
    /// 已完成的静态面板。
    pub panel: CharacterPanel,
    /// 面板各项数值的来源。
    pub panel_contributions: Vec<PanelContribution>,
    /// 后续战斗与机制处理需要的引用。
    pub mechanisms: Vec<MechanismReference>,
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
    /// 确认持有的原始光锥库存；是否允许使用由模式与玩法约束决定。
    pub owned_light_cones: Vec<LightConeInstance>,
    /// 确认持有的原始遗器库存。
    pub owned_relics: Vec<RelicInstance>,
}
