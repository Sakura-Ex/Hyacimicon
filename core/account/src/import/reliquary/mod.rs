//! 导入 reliquary-archiver v4 的原始账号。
//!
//! 来源语义依据导出器 v0.19.0 的格式与转换实现；行迹绑定使用调用方
//! 提供的版本快照。调用方接收统一账号和导入报告。

mod equipment;
mod source;
mod traces;

use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{CharacterConfig, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount};

/// 一次成功导入的账号和报告。
#[derive(Debug, Serialize)]
pub struct ReliquaryImport {
    /// 项目统一的原始账号，供后续配置转换使用。
    pub account: RawAccount,
    /// 来源版本、行迹依据和缺失信息。
    pub report: ImportReport,
}

/// 导入依据与需要补齐的信息，独立于原始账号保存。
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportReport {
    /// 来源格式名称。
    pub source: String,
    /// 来源导出器构建版本。
    pub build: String,
    /// 来源格式版本。
    pub format_version: u32,
    /// 行迹标签到真实节点 ID 的绑定快照。
    pub trace_bindings_snapshot: String,
    /// 需要后续补齐或适配的字段。
    pub warnings: Vec<ImportWarning>,
}

/// 来源缺失或扩展字段的位置和说明。
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportWarning {
    /// 来源字段路径。
    pub path: String,
    /// 需要补齐或适配的内容及其影响。
    pub message: String,
}

/// JSON 解析或字段转换失败，失败时返回具体原因。
#[derive(Debug)]
pub enum ImportError {
    /// 来源 JSON 的语法或字段类型不符合要求。
    Json(serde_json::Error),
    /// 字段值、引用或格式版本不受支持。
    Field {
        /// 来源字段路径。
        path: String,
        /// 错误原因。
        message: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "导出 JSON 解析失败：{error}"),
            Self::Field { path, message } => write!(formatter, "{path}：{message}"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Field { .. } => None,
        }
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// 导入一个完整的 reliquary-archiver v4 JSON。
///
/// `config_id` 由调用方分配，同时作为装备实例 ID 的命名空间。成功时保留
/// 原始练度和完整库存；未知行迹留空并写入报告。游戏版本与完整游戏快照由
/// 后续配置转换入口关联，本函数将这两个字段保留为 `None`。
/// `catalog` 是调用方已加载的对应版本行迹资料。
///
/// 错误覆盖格式标识、字段类型、支持范围、重复 ID、装备归属与未知属性键。
pub fn import_reliquary_archive(
    input: &str,
    config_id: &str,
    catalog: &crate::reference::TraceCatalog,
) -> Result<ReliquaryImport, ImportError> {
    if config_id.trim().is_empty() {
        return Err(invalid("config_id", "配置标识应包含有效字符"));
    }
    let header: source::Header = serde_json::from_str(input)?;
    if header.source != "reliquary_archiver" {
        return Err(invalid("source", "支持的来源为 reliquary_archiver"));
    }
    if header.version != 4 {
        return Err(invalid("version", "当前支持 reliquary-archiver 格式版本 4"));
    }
    let export: source::Export = serde_json::from_str(input)?;
    if export.build.trim().is_empty() {
        return Err(invalid("build", "导出器构建版本缺失"));
    }
    let mut report = ImportReport {
        source: export.source.clone(),
        build: export.build.clone(),
        format_version: export.version,
        trace_bindings_snapshot: catalog.snapshot_id.clone(),
        warnings: Vec::new(),
    };
    source::report_extensions(&export, &mut report);
    let mut characters = Vec::with_capacity(export.characters.len());
    let mut character_indices = BTreeMap::new();
    for (index, character) in export.characters.iter().enumerate() {
        let path = format!("characters[{index}]");
        numeric_id(&character.id, &format!("{path}.id"))?;
        if character_indices
            .insert(character.id.clone(), index)
            .is_some()
        {
            return Err(invalid(format!("{path}.id"), "角色 ID 重复"));
        }
        progression(character.level, character.ascension, &path)?;
        bounded(character.eidolon, 0, 6, &format!("{path}.eidolon"))?;
        characters.push(CharacterConfig {
            character_id: character.id.clone(),
            level: character.level,
            promotion: character.ascension,
            eidolons: character.eidolon,
            ability_version: Some(character.ability_version),
            traces: catalog.convert(character, &path, &mut report)?,
            light_cone_instance_id: None,
            relic_instance_ids: BTreeMap::new(),
        });
    }
    let (light_cones, relics) =
        equipment::convert(&export, config_id, &character_indices, &mut characters)?;
    Ok(ReliquaryImport {
        account: RawAccount {
            schema_version: RAW_ACCOUNT_SCHEMA_VERSION,
            config_id: config_id.to_owned(),
            game_version: None,
            data_snapshot: None,
            characters,
            light_cones,
            relics,
        },
        report,
    })
}

/// 为来源字段构造可定位的转换错误。
fn invalid(path: impl Into<String>, message: impl Into<String>) -> ImportError {
    ImportError::Field {
        path: path.into(),
        message: message.into(),
    }
}

/// 检查来源中的正整数字符串 ID，保持其字符串表示。
fn numeric_id(value: &str, path: &str) -> Result<(), ImportError> {
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.parse::<u32>().is_err()
    {
        return Err(invalid(path, "ID 应为有效的正整数字符串"));
    }
    Ok(())
}

/// 检查当前来源版本的整数取值范围。
fn bounded(value: u8, min: u8, max: u8, path: &str) -> Result<(), ImportError> {
    if !(min..=max).contains(&value) {
        return Err(invalid(path, format!("数值应在 {min}～{max} 范围内")));
    }
    Ok(())
}

/// 检查角色及光锥等级和突破阶段是否相容。
fn progression(level: u8, promotion: u8, path: &str) -> Result<(), ImportError> {
    bounded(promotion, 0, 6, &format!("{path}.ascension"))?;
    let min = if promotion == 0 {
        1
    } else {
        10 + promotion * 10
    };
    bounded(level, min, 20 + promotion * 10, &format!("{path}.level"))
}
