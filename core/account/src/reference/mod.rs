//! 从项目文档整理的 4.7 S2 数据与试用、映射模板。

pub mod models;
pub mod templates;
pub mod traces;

pub use models::*;
pub use templates::*;
pub use traces::*;

use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use crate::{RelicMainStat, playable::attributes::AttributeValues};

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
    /// 各角色小行迹节点及其属性贡献。
    pub minor_traces: Vec<CharacterMinorTraces>,
    /// 共用的行迹节点与星魂技能加级资料。
    pub traces: TraceCatalog,
    /// 套装达到指定件数时的常驻属性。
    pub set_properties: BTreeMap<String, BTreeMap<u8, AttributeValues>>,
    /// 五星 +15 主词条数值。
    pub mainstats: BTreeMap<RelicMainStat, f64>,
}

/// A missing or invalid reference file, including its path.
#[derive(Debug)]
pub struct ReferenceLoadError {
    /// The file that could not be loaded.
    pub path: PathBuf,
    /// The original I/O or deserialization error.
    pub cause: Box<dyn std::error::Error + Send + Sync>,
}

impl fmt::Display for ReferenceLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.cause)
    }
}

impl std::error::Error for ReferenceLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ReferenceLoadError> {
    let source = fs::read(path).map_err(|cause| ReferenceLoadError {
        path: path.to_owned(),
        cause: Box::new(cause),
    })?;
    serde_json::from_slice(&source).map_err(|cause| ReferenceLoadError {
        path: path.to_owned(),
        cause: Box::new(cause),
    })
}

impl PlayableData {
    /// Loads one versioned playable snapshot with an explicitly selected trace catalog.
    pub fn from_dir(dir: &Path, trace_catalog: &Path) -> Result<Self, ReferenceLoadError> {
        Ok(Self {
            manifest: read_json(&dir.join("manifest.json"))?,
            characters: read_json(&dir.join("characters.json"))?,
            light_cones: read_json(&dir.join("light-cones.json"))?,
            trials: read_json(&dir.join("trial-templates.json"))?,
            mappings: read_json(&dir.join("mapping-templates.json"))?,
            default_light_cones: read_json(&dir.join("default-light-cones.json"))?,
            minor_traces: read_json(&dir.join("minor-traces.json"))?,
            traces: TraceCatalog::from_path(trace_catalog)?,
            set_properties: read_json(&dir.join("set-properties.json"))?,
            mainstats: read_json(&dir.join("mainstats.json"))?,
        })
    }
}
