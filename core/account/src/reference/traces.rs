//! 导入与实际配置转换共用的节点、技能及星魂加级资料。

use std::collections::BTreeMap;

use serde::Deserialize;

/// 行迹节点及其控制的技能。
#[derive(Debug, Deserialize)]
pub struct TraceNode {
    /// 行迹节点 ID。
    pub node_id: String,
    /// 星魂加级前的等级上限。
    pub max_level: u8,
    /// 该节点同时升级的实际技能 ID。
    #[serde(default)]
    pub skill_ids: Vec<String>,
}

/// 一个角色能力版本的行迹绑定。
#[derive(Debug, Deserialize)]
pub struct TraceBinding {
    /// 常规角色 ID。
    pub character_id: String,
    /// 能力版本。
    pub ability_version: u32,
    /// 技能类别到节点的映射，一级上限技能固定为一级。
    pub skills: BTreeMap<String, TraceNode>,
    /// 一次性行迹标签到节点的映射。
    pub nodes: BTreeMap<String, TraceNode>,
    /// 星魂序号到技能 ID 与附加等级的映射。
    pub rank_bonuses: BTreeMap<u8, BTreeMap<String, u8>>,
}

/// 随程序提供的版本化行迹资料。
#[derive(Debug, Deserialize)]
pub struct TraceCatalog {
    /// 资料快照标识。
    pub snapshot_id: String,
    /// 角色与能力版本的绑定记录。
    pub characters: Vec<TraceBinding>,
}

impl TraceCatalog {
    /// 读取内置行迹资料。
    pub fn bundled() -> Result<Self, serde_json::Error> {
        serde_json::from_str(include_str!("../../data/reliquary-traces-v4.json"))
    }

    /// 查找某个角色能力版本。
    pub fn binding(&self, character_id: &str, version: u32) -> Option<&TraceBinding> {
        self.characters
            .iter()
            .find(|entry| entry.character_id == character_id && entry.ability_version == version)
    }

    /// 查找玩法采用的最高能力版本。
    pub fn latest(&self, character_id: &str) -> Option<&TraceBinding> {
        self.characters
            .iter()
            .filter(|entry| entry.character_id == character_id)
            .max_by_key(|entry| entry.ability_version)
    }
}
