//! 试用与映射模板保留文档中的技能等级、属性合计和缺省字段。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::models::GamePath;
use crate::{
    RelicSlot,
    playable::attributes::{Attribute, AttributeValues},
};

/// 模板中的技能类别，实际节点 ID 在配置展开时绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// 普攻。
    Basic,
    /// 战技。
    Skill,
    /// 终结技。
    Ultimate,
    /// 天赋。
    Talent,
    /// 忆灵战技。
    MemospriteSkill,
    /// 忆灵天赋。
    MemospriteTalent,
    /// 欢愉技。
    Elation,
    /// 阿哈正传。
    AhaBiography,
}

/// 模板提供的行迹基础等级与未激活小行迹节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceTemplate {
    /// 星魂加级前的技能等级。
    pub base_levels: BTreeMap<SkillKind, u8>,
    /// 未激活小行迹的后三位节点 ID，对应角色小行迹表的 201～210。
    pub inactive_minor_nodes: Vec<u16>,
}

/// 模板中的光锥定义，等级和突破保留文档中的未知状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConeTemplate {
    /// 光锥种类 ID。
    pub light_cone_id: String,
    /// 叠影等级。
    pub superimposition: u8,
    /// 等级，未提供时为空。
    pub level: Option<u8>,
    /// 突破阶段，未提供时为空。
    pub promotion: Option<u8>,
}

/// 试用表直接提供的六件遗器属性合计。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialRelics {
    /// 外圈套装 ID。
    pub outer_set_id: String,
    /// 内圈套装 ID。
    pub planar_set_id: String,
    /// 主副词条合计，套装效果另行处理。
    pub stats: AttributeValues,
}

/// 文档中的试用配置，供未持有角色和全试用任务使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialTemplate {
    /// 常规或玩法专属角色 ID。
    pub character_id: String,
    /// 试用星魂数。
    pub eidolons: u8,
    /// 试用角色等级，文档未提供时为空。
    pub character_level: Option<u8>,
    /// 试用角色突破，文档未提供时为空。
    pub character_promotion: Option<u8>,
    /// 行迹配置；文档明确无对应条目时为空。
    pub traces: Option<TraceTemplate>,
    /// 试用光锥。
    pub light_cone: LightConeTemplate,
    /// 六件遗器合计。
    pub relics: TrialRelics,
}

/// 内圈和外圈的替换范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelicGroup {
    /// 头、手、躯干、脚。
    Outer,
    /// 位面球、连结绳。
    Planar,
}

/// 一圈遗器被整体替换后的套装与属性合计。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicGroupTemplate {
    /// 套装 ID。
    pub set_id: String,
    /// 此圈主副词条合计。
    pub stats: AttributeValues,
}

/// 文档记录的角色映射目标，各部分的触发条件由规则决定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingTemplate {
    /// 角色 ID。
    pub character_id: String,
    /// 映射后的星魂数。
    pub eidolons: u8,
    /// 映射后的基础行迹。
    pub traces: TraceTemplate,
    /// 外圈整体替换配置。
    pub outer: RelicGroupTemplate,
    /// 内圈整体替换配置。
    pub planar: RelicGroupTemplate,
}

/// 分别判断是否映射的配置部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingPart {
    /// 角色等级与突破。
    CharacterProgression,
    /// 星魂。
    Eidolons,
    /// 行迹。
    Traces,
    /// 光锥。
    LightCone,
    /// 外圈遗器。
    OuterRelics,
    /// 内圈遗器。
    PlanarRelics,
}

/// 按命途选择的默认映射光锥。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultLightCone {
    /// 命途。
    pub path: GamePath,
    /// 默认光锥种类 ID。
    pub light_cone_id: String,
}

/// 配置转换使用的规则参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingRules {
    /// 有强化形态时固定使用强化形态，具体版本通过角色资料解析。
    pub force_enhanced_form: bool,
    /// 任意一件低于此强化等级时，整圈采用映射模板。
    pub relic_mapping_minimum_level: u8,
    /// 默认映射光锥的叠影。
    pub default_light_cone_superimposition: u8,
    /// 各命途默认光锥。
    pub default_light_cones: Vec<DefaultLightCone>,
}

/// 主词条在文档指定品质和强化等级下的数值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MainStatData {
    /// 属性键。
    pub attribute: Attribute,
    /// 允许的部位。
    pub slots: Vec<RelicSlot>,
    /// 最终数值。
    pub value: f64,
}

/// 一种副词条的低、中、高三档数值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubstatData {
    /// 属性键。
    pub attribute: Attribute,
    /// 低、中、高档，百分比采用比例。
    pub values: [f64; 3],
}

/// 遗器套装 ID、显示名与内外圈归属。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicSetData {
    /// 套装 ID。
    pub id: String,
    /// 所属内外圈。
    pub group: RelicGroup,
}

/// 文档覆盖的遗器数值与套装索引。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicReference {
    /// 主词条数据适用星级。
    pub rarity: u8,
    /// 主词条数据适用强化等级。
    pub level: u8,
    /// 主词条表。
    pub mainstats: Vec<MainStatData>,
    /// 副词条档位表。
    pub substats: Vec<SubstatData>,
    /// 套装索引，效果机制另行维护。
    pub sets: Vec<RelicSetData>,
}
