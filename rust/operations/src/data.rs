use crate::models::{
    AffixDefinition, AffixEffect, DifficultyDefinition, EnvironmentDefinition,
    InvestmentDefinition, NodeDefinition, NodeType, OperationRules, Position, RulePack,
    TraitDefinition, TraitTier, UnitDefinition,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

const SNAPSHOT_FILES: [&str; 6] = [
    "affixes",
    "characters",
    "traits",
    "trait_details",
    "presentation",
    "numerics",
];

#[derive(Debug, Deserialize)]
struct BasePack {
    difficulties: Vec<DifficultyDefinition>,
    affixes: Vec<AffixDefinition>,
    environments: Vec<EnvironmentDefinition>,
    investments: Vec<InvestmentDefinition>,
}

#[derive(Debug, Deserialize)]
struct CharacterRow {
    id: String,
    name: String,
    description: String,
    cost: Vec<u8>,
    roles: Vec<String>,
    traits: Vec<String>,
    position: String,
}

#[derive(Debug, Deserialize)]
struct TraitRow {
    id: String,
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct AffixRow {
    id: String,
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    id: String,
    plane: u8,
    kind: String,
}

pub fn discover_data_root() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("HSR_CURRENCY_WARS_DATA") {
        let path = PathBuf::from(value);
        if path.join("data/encyclopedia/manifest.json").is_file() {
            return Ok(path);
        }
    }
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join("hsr-currency-wars-data");
        if candidate.join("data/encyclopedia/manifest.json").is_file() {
            return Ok(candidate);
        }
    }
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in crate_root.ancestors() {
        let candidate = ancestor.join("hsr-currency-wars-data");
        if candidate.join("data/encyclopedia/manifest.json").is_file() {
            return Ok(candidate);
        }
    }
    Err("未找到 hsr-currency-wars-data 数据仓库".into())
}

pub fn load_rule_pack(data_root: &Path) -> Result<RulePack, String> {
    let encyclopedia = data_root.join("data/encyclopedia");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(encyclopedia.join("manifest.json"))
            .map_err(|error| format!("读取快照清单失败：{error}"))?,
    )
    .map_err(|error| format!("解析快照清单失败：{error}"))?;
    let mut snapshot = HashMap::new();
    for name in SNAPSHOT_FILES {
        snapshot.insert(name, read_verified_json(data_root, &manifest, name)?);
    }

    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let base_path = crate_root.join("../../research/operations/data/default_pack.json");
    let base_text = fs::read_to_string(&base_path)
        .map_err(|error| format!("读取基础规则包失败 {}：{error}", base_path.display()))?;
    let base: BasePack =
        serde_json::from_str(&base_text).map_err(|error| format!("解析基础规则包失败：{error}"))?;
    let base_value: Value = serde_json::from_str(&base_text).map_err(|error| error.to_string())?;

    let affix_rows: Vec<AffixRow> = serde_json::from_value(snapshot["affixes"].clone())
        .map_err(|error| format!("解析最新词缀失败：{error}"))?;
    let effects_by_name: HashMap<String, AffixEffect> = base
        .affixes
        .iter()
        .map(|affix| (affix.name.clone(), affix.effect.clone().normalized()))
        .collect();
    let affixes = affix_rows
        .into_iter()
        .map(|row| AffixDefinition {
            id: row.id,
            name: row.name.clone(),
            description: row.description,
            effect: effects_by_name
                .get(&row.name)
                .cloned()
                .unwrap_or_default()
                .normalized(),
        })
        .collect();

    let trait_rows: Vec<TraitRow> = serde_json::from_value(snapshot["traits"].clone())
        .map_err(|error| format!("解析最新羁绊失败：{error}"))?;
    let trait_details = snapshot["trait_details"]
        .get("traits")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let presentation = snapshot["presentation"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let trait_images = presentation
        .get("images")
        .and_then(Value::as_object)
        .and_then(|images| images.get("traits"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let trait_groups = presentation
        .get("trait_groups")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut traits = HashMap::new();
    let mut trait_id_by_name = HashMap::new();
    for row in trait_rows {
        let detail = trait_details
            .get(&row.id)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let tiers = detail
            .get("tiers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| {
                Some(TraitTier {
                    level: value.get("level")?.as_i64()? as i32,
                    text: value.get("text")?.as_str()?.to_owned(),
                    quality: value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("None")
                        .to_owned(),
                })
            })
            .collect();
        let members = detail
            .get("members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect();
        let icon_path = trait_images
            .get(&row.id)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let group = trait_groups
            .get(&row.id)
            .and_then(Value::as_str)
            .unwrap_or("其他")
            .to_owned();
        trait_id_by_name.insert(row.name.clone(), row.id.clone());
        traits.insert(
            row.id.clone(),
            TraitDefinition {
                id: row.id,
                name: row.name,
                description: row.description,
                group,
                icon_path,
                tiers,
                members,
            },
        );
    }

    let character_rows: Vec<CharacterRow> = serde_json::from_value(snapshot["characters"].clone())
        .map_err(|error| format!("解析最新角色失败：{error}"))?;
    let character_presentation = presentation
        .get("characters")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut units = HashMap::new();
    for row in character_rows {
        let position = Position::parse(&row.position)?;
        let portrait_path = character_presentation
            .get(&row.id)
            .and_then(Value::as_object)
            .and_then(|value| value.get("portrait"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let trait_ids = row
            .traits
            .iter()
            .map(|name| {
                trait_id_by_name
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("角色 {} 引用了未知羁绊 {name}", row.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cost = *row
            .cost
            .iter()
            .min()
            .ok_or_else(|| format!("角色 {} 缺少费用", row.name))?;
        units.insert(
            row.id.clone(),
            UnitDefinition {
                id: row.id,
                name: row.name,
                description: row.description,
                cost,
                costs: row.cost,
                position,
                roles: row.roles,
                traits: row.traits,
                trait_ids,
                portrait_path,
            },
        );
    }

    let node_rows: Vec<NodeRow> = serde_json::from_value(
        snapshot["numerics"]
            .get("nodes")
            .cloned()
            .ok_or_else(|| "数值快照缺少节点".to_string())?,
    )
    .map_err(|error| format!("解析节点失败：{error}"))?;
    let nodes = node_rows
        .into_iter()
        .map(|row| {
            Ok(NodeDefinition {
                id: row.id,
                plane: row.plane,
                node_type: NodeType::parse(&row.kind)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if nodes.is_empty() {
        return Err("节点列表不能为空".into());
    }

    let assumptions = base_value
        .get("metadata")
        .and_then(|value| value.get("assumptions"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .chain(std::iter::once(
            "角色、羁绊、词缀与节点顺序来自私有数据仓库图鉴快照。".to_owned(),
        ))
        .collect();
    let snapshot_id = required_string(&manifest, "snapshot_id")?;
    let content_sha256 = required_string(&manifest, "content_sha256")?;
    let game_version = required_string(&manifest, "game_version")?;
    let season = required_string(&manifest, "season")?;
    let rules = OperationRules {
        game_version: game_version.clone(),
        ..OperationRules::default()
    };

    Ok(RulePack {
        game_version,
        snapshot_id,
        content_sha256,
        validation_status: format!("{season} 图鉴快照，已校验 SHA256"),
        assumptions,
        rules,
        units,
        traits,
        affixes,
        difficulties: base.difficulties,
        environments: base
            .environments
            .into_iter()
            .filter(|environment| environment.effect.starting_unit_count == 0)
            .map(|mut environment| {
                environment.effect = environment.effect.normalized();
                environment
            })
            .collect(),
        investments: base
            .investments
            .into_iter()
            .map(|mut investment| {
                investment.effect = investment.effect.normalized();
                investment
            })
            .collect(),
        nodes,
    })
}

fn read_verified_json(data_root: &Path, manifest: &Value, name: &str) -> Result<Value, String> {
    let entry = manifest
        .get("files")
        .and_then(|value| value.get(name))
        .ok_or_else(|| format!("快照清单缺少 {name}"))?;
    let relative = required_string(entry, "path")?;
    let expected = required_string(entry, "sha256")?;
    let path = data_root.join(&relative);
    let canonical_root = data_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("读取快照文件失败 {}：{error}", path.display()))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!("快照路径越界：{relative}"));
    }
    let bytes = fs::read(&canonical).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(&bytes);
    let mut actual = String::new();
    for byte in digest {
        write!(&mut actual, "{byte:02x}").expect("写入哈希字符串");
    }
    if actual != expected {
        return Err(format!("快照文件校验失败：{name}"));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("解析快照文件 {name} 失败：{error}"))
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("缺少字段 {key}"))
}
