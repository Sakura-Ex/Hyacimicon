//! 角色原始练度、行迹和装备引用。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::RelicSlot;

/// 按真实行迹节点 ID 保存的原始行迹。
///
/// 节点 ID 通过对应角色、能力版本和资料快照绑定。技能等级采用星魂加级前的
/// 基础等级；普攻、秘技、忆灵技能和欢愉技等使用同一个节点映射。
/// 缺少的键表示该节点信息未知，接入层须按资料检查所需节点是否齐全。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceConfig {
    /// 技能类行迹节点 ID 到基础等级的映射。
    pub skill_levels: BTreeMap<String, u8>,
    /// 额外能力、属性等一次性节点的解锁状态；`false` 明确表示未解锁。
    pub node_unlocks: BTreeMap<String, bool>,
}

/// 一种持有角色形态的原始练度和装备关系。
///
/// 角色 ID 区分开拓者、三月七等不同形态。等级和行迹保存原始值，
/// 对应玩法中的试用配置与练度保底在后续转换时应用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterConfig {
    /// 常规角色 ID，使用字符串以沿用游戏资料的 ID 口径。
    pub character_id: String,
    /// 原始角色等级。
    pub level: u8,
    /// 原始突破阶段，当前资料取 0～6。
    pub promotion: u8,
    /// 原始星魂数，当前资料取 0～6。
    pub eidolons: u8,
    /// 能力配置版本的来源标识；未知时为 `None`，已知的 0 保留为 `Some(0)`。
    pub ability_version: Option<u32>,
    /// 基础行迹等级与一次性节点的已知解锁状态。
    pub traces: TraceConfig,
    /// 所装备光锥的库存实例 ID；`None` 表示未装备。
    pub light_cone_instance_id: Option<String>,
    /// 按部位引用遗器库存实例；缺少的部位表示未装备。
    pub relic_instance_ids: BTreeMap<RelicSlot, String>,
}
