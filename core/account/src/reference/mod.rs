//! 从项目文档整理的 4.7 S2 数据与试用、映射模板。

pub mod models;
pub mod templates;

pub use models::*;
pub use templates::*;

use std::collections::BTreeMap;

/// 账号转换使用的基础数据、模板与规则。
#[derive(Debug)]
pub struct PlayableData {
    /// 游戏版本、赛季与快照标识。
    pub manifest: DataManifest,
    /// 角色自身基础数值。
    pub characters: Vec<CharacterData>,
    /// 光锥自身基础数值。
    pub light_cones: Vec<LightConeData>,
    /// 试用配置。
    pub trials: Vec<TrialTemplate>,
    /// 映射目标配置。
    pub mappings: Vec<MappingTemplate>,
    /// 各命途的默认映射光锥 ID。
    pub default_light_cones: BTreeMap<GamePath, String>,
    /// 局内星级参数，供后续组装阶段选择。
    pub gameplay_forms: Vec<GameplayFormData>,
    /// 各角色小行迹节点及其属性贡献。
    pub minor_traces: Vec<CharacterMinorTraces>,
}

impl PlayableData {
    /// 加载随 crate 内置的文档数据，解析失败时返回原始错误。
    pub fn bundled() -> Result<Self, serde_json::Error> {
        Ok(Self {
            manifest: serde_json::from_str(include_str!("../../data/s2-v4.7/manifest.json"))?,
            characters: serde_json::from_str(include_str!("../../data/s2-v4.7/characters.json"))?,
            light_cones: serde_json::from_str(include_str!("../../data/s2-v4.7/light-cones.json"))?,
            trials: serde_json::from_str(include_str!("../../data/s2-v4.7/trial-templates.json"))?,
            mappings: serde_json::from_str(include_str!(
                "../../data/s2-v4.7/mapping-templates.json"
            ))?,
            default_light_cones: serde_json::from_str(include_str!(
                "../../data/s2-v4.7/default-light-cones.json"
            ))?,
            gameplay_forms: serde_json::from_str(include_str!(
                "../../data/s2-v4.7/gameplay-forms.json"
            ))?,
            minor_traces: serde_json::from_str(include_str!(
                "../../data/s2-v4.7/minor-traces.json"
            ))?,
        })
    }
}
