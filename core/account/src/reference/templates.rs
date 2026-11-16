//! 试用与映射模板保留文档中的技能等级、属性合计和缺省字段。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::playable::attributes::AttributeValues;

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

/// 文档中的试用配置，供未持有角色使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialTemplate {
    /// 常规或玩法专属角色 ID。
    pub character_id: String,
    /// 试用星魂数。
    pub eidolons: u8,
    /// 行迹配置；文档明确无对应条目时为空。
    pub traces: Option<TraceTemplate>,
    /// 试用光锥种类 ID。
    pub light_cone_id: String,
    /// 六件遗器合计。
    pub relics: TrialRelics,
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

/// 每个角色独立的内外圈遗器映射目标。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingTemplate {
    /// 角色 ID。
    pub character_id: String,
    /// 外圈整体替换配置。
    pub outer: RelicGroupTemplate,
    /// 内圈整体替换配置。
    pub planar: RelicGroupTemplate,
}
