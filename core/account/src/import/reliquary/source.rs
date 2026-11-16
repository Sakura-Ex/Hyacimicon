//! reliquary-archiver v4 的计算字段；扩展字段仅报告路径。

use std::collections::BTreeMap;

use serde::{Deserialize, de::IgnoredAny};

use super::{ImportReport, ImportWarning};

/// 先读取格式标识，使不支持的版本得到明确错误。
#[derive(Deserialize)]
pub(super) struct Header {
    pub source: String,
    pub version: u32,
}

/// 参与转换的顶层字段及其余来源内容。
#[derive(Deserialize)]
pub(super) struct Export {
    pub source: String,
    pub build: String,
    pub version: u32,
    pub characters: Vec<Character>,
    pub light_cones: Vec<LightCone>,
    pub relics: Vec<Relic>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, IgnoredAny>,
}

/// 来源角色；技能等级直接来自账号技能树。
#[derive(Deserialize)]
pub(super) struct Character {
    pub id: String,
    pub level: u8,
    pub ascension: u8,
    pub eidolon: u8,
    pub ability_version: u32,
    pub skills: BTreeMap<String, u8>,
    pub traces: BTreeMap<String, bool>,
    pub memosprite: Option<BTreeMap<String, u8>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, IgnoredAny>,
}

/// 来源光锥，装备归属通过 `location` 引用角色。
#[derive(Deserialize)]
pub(super) struct LightCone {
    pub id: String,
    #[serde(rename = "_uid")]
    pub uid: String,
    pub level: u8,
    pub ascension: u8,
    pub superimposition: u8,
    pub location: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, IgnoredAny>,
}

/// 来源遗器，当前与预览、重掷词条分别保存。
#[derive(Deserialize)]
pub(super) struct Relic {
    #[serde(rename = "_uid")]
    pub uid: String,
    pub set_id: String,
    pub slot: String,
    pub rarity: u8,
    pub level: u8,
    pub mainstat: String,
    pub substats: Vec<Substat>,
    pub location: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, IgnoredAny>,
}

/// 来源当前副词条，百分比数值使用百分数。
#[derive(Deserialize)]
pub(super) struct Substat {
    pub key: String,
    pub value: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, IgnoredAny>,
}

/// 标注超出已核对 v4 格式的扩展字段，供调用方确认其影响。
pub(super) fn report_extensions(export: &Export, report: &mut ImportReport) {
    extension_fields(
        &export.extra,
        "",
        &["metadata", "gacha", "materials"],
        report,
    );
    for (index, character) in export.characters.iter().enumerate() {
        extension_fields(
            &character.extra,
            &format!("characters[{index}]"),
            &["name", "path"],
            report,
        );
    }
    for (index, cone) in export.light_cones.iter().enumerate() {
        extension_fields(
            &cone.extra,
            &format!("light_cones[{index}]"),
            &["name", "lock"],
            report,
        );
    }
    for (index, relic) in export.relics.iter().enumerate() {
        let path = format!("relics[{index}]");
        extension_fields(
            &relic.extra,
            &path,
            &[
                "name",
                "lock",
                "discard",
                "preview_substats",
                "reroll_substats",
            ],
            report,
        );
        for (index, stat) in relic.substats.iter().enumerate() {
            extension_fields(
                &stat.extra,
                &format!("{path}.substats[{index}]"),
                &["count", "step"],
                report,
            );
        }
    }
}

/// 报告尚未参与统一配置转换的字段路径。
fn extension_fields(
    extra: &BTreeMap<String, IgnoredAny>,
    path: &str,
    known: &[&str],
    report: &mut ImportReport,
) {
    for name in extra.keys().filter(|name| !known.contains(&name.as_str())) {
        report.warnings.push(ImportWarning {
            path: if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            },
            message: "来源扩展字段未参与转换，其配置含义待适配".to_owned(),
        });
    }
}
