//! 从项目文档整理的 4.7 S2 数据与试用、映射模板。

pub mod models;
pub mod templates;

pub use models::*;
pub use templates::*;

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
    /// 默认光锥和已记录的映射规则。
    pub rules: MappingRules,
    /// 局内星级参数，供后续组装阶段选择。
    pub gameplay_forms: Vec<GameplayFormData>,
    /// 遗器数值与套装索引。
    pub relics: RelicReference,
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
            rules: serde_json::from_str(include_str!("../../data/s2-v4.7/mapping-rules.json"))?,
            gameplay_forms: serde_json::from_str(include_str!(
                "../../data/s2-v4.7/gameplay-forms.json"
            ))?,
            relics: serde_json::from_str(include_str!("../../data/s2-v4.7/relics.json"))?,
        })
    }
}
