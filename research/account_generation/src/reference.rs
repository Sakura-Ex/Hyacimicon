use std::{collections::BTreeMap, fs, path::Path};

use account_core::{RelicMainStat, RelicSubStat, TraceConfig, reference::traces::TraceCatalog};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{CharacterSpec, GenerationError, GoldCategory};

const BUNDLED_PROFILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/profiles.rs"));

/// 可复用的同版本生成资料，包含角色、光锥、推荐配装和行迹条件。
#[derive(Debug)]
pub struct AccountReference {
    pub(crate) game_version: String,
    pub(crate) data_snapshot: Option<String>,
    pub(crate) characters: BTreeMap<String, Profile>,
    pub(crate) light_cones: BTreeMap<String, Cone>,
}

/// 同一份资料中显式指定的金数类别、专锥及能力版本。
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReferenceOverrides {
    pub character_categories: BTreeMap<String, GoldCategory>,
    pub light_cone_categories: BTreeMap<String, GoldCategory>,
    pub signature_light_cones: BTreeMap<String, String>,
    pub ability_versions: BTreeMap<String, u32>,
}

#[derive(Debug)]
pub(crate) struct Profile {
    pub id: String,
    pub rarity: u8,
    pub path: String,
    pub category: GoldCategory,
    pub ability_version: Option<u32>,
    pub signature: Option<String>,
    pub recommended: Vec<String>,
    pub teams: Vec<Team>,
    pub relics: Recommendations,
    traces: BTreeMap<String, BTreeMap<u8, TraceLevel>>,
}

#[derive(Debug)]
pub(crate) struct Cone {
    pub id: String,
    pub path: String,
    pub category: GoldCategory,
    pub unique: bool,
}

#[derive(Debug)]
pub(crate) struct Recommendations {
    pub mains: [Vec<RelicMainStat>; 6],
    pub weights: BTreeMap<RelicSubStat, f64>,
    pub outer: Vec<String>,
    pub planar: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Team {
    pub members: Vec<String>,
    pub backups: [Vec<String>; 3],
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    catalog_snapshot: Option<String>,
}

#[derive(Deserialize)]
struct ConeSource {
    rank: String,
    #[serde(rename = "baseType")]
    path: String,
}

#[derive(Deserialize)]
struct ProfileSource {
    rarity: String,
    base_type: String,
    lightcones: Vec<u32>,
    relics: RelicSource,
    teams: Vec<TeamSource>,
    skill_trees: BTreeMap<String, BTreeMap<u8, TraceLevel>>,
}

#[derive(Deserialize)]
struct RelicSource {
    property_list3: Vec<String>,
    property_list4: Vec<String>,
    property_list5: Vec<String>,
    property_list6: Vec<String>,
    sub_affix_property_list: Vec<String>,
    set4_id_list: Vec<u32>,
    set2_id_list: Vec<u32>,
}

#[derive(Deserialize)]
struct TeamSource {
    member_list: Vec<u32>,
    backup_list1: Vec<u32>,
    backup_list2: Vec<u32>,
    backup_list3: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct TraceLevel {
    point_id: u32,
    avatar_level_limit: Option<u8>,
    avatar_promotion_limit: Option<u8>,
    level_up_skill_id: Vec<u32>,
    pre_point: Vec<u32>,
}

impl AccountReference {
    /// 加载随程序提供的 4.7 生成资料。
    pub fn bundled() -> Result<Self, GenerationError> {
        Self::bundled_with(&ReferenceOverrides::default())
    }

    /// 加载内置资料并覆盖指定的生成用分类、专锥或版本。
    pub fn bundled_with(overrides: &ReferenceOverrides) -> Result<Self, GenerationError> {
        Self::load(
            include_str!("../data/source-manifest.json"),
            include_str!("../data/character.json"),
            include_str!("../data/lightcone.json"),
            |id| {
                BUNDLED_PROFILES
                    .iter()
                    .find(|(key, _)| *key == id)
                    .ok_or_else(|| error(format!("缺少角色 {id} 的内置生成资料")))
                    .and_then(|(_, source)| parse(source, id))
            },
            overrides,
        )
    }

    /// 加载外部资料目录，文件格式与内置资料相同。
    pub fn from_dir(path: &Path, overrides: &ReferenceOverrides) -> Result<Self, GenerationError> {
        Self::load(
            &read(&path.join("source-manifest.json"))?,
            &read(&path.join("character.json"))?,
            &read(&path.join("lightcone.json"))?,
            |id| {
                if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(error(format!("无效的常规角色 ID {id}")));
                }
                parse(
                    &read(&path.join("character").join(format!("{id}.json")))?,
                    id,
                )
            },
            overrides,
        )
    }

    fn load(
        manifest: &str,
        characters: &str,
        cones: &str,
        profile: impl Fn(&str) -> Result<ProfileSource, GenerationError>,
        overrides: &ReferenceOverrides,
    ) -> Result<Self, GenerationError> {
        let manifest: Manifest = parse(manifest, "source-manifest.json")?;
        let catalog: BTreeMap<String, serde::de::IgnoredAny> = parse(characters, "character.json")?;
        let cones: BTreeMap<String, ConeSource> = parse(cones, "lightcone.json")?;
        let mut light_cones = BTreeMap::new();
        for (id, source) in cones {
            let rarity = rarity(&source.rank)?;
            let category = overrides
                .light_cone_categories
                .get(&id)
                .copied()
                .unwrap_or_else(|| {
                    category(
                        rarity,
                        &id,
                        "24",
                        &[
                            "23000", "23002", "23003", "23004", "23005", "23012", "23013",
                        ],
                    )
                });
            light_cones.insert(
                id.clone(),
                Cone {
                    unique: id.starts_with("22") || id.starts_with("24"),
                    id,
                    path: source.path,
                    category,
                },
            );
        }
        let trace_catalog = if manifest.version == "4.7" {
            Some(TraceCatalog::bundled().map_err(|cause| error(cause.to_string()))?)
        } else {
            None
        };
        let mut characters = BTreeMap::new();
        for id in catalog.into_keys() {
            let source = profile(&id)?;
            let rarity = rarity(&source.rarity)?;
            let category = overrides
                .character_categories
                .get(&id)
                .copied()
                .unwrap_or_else(|| {
                    category(
                        rarity,
                        &id,
                        "80",
                        &["1003", "1004", "1101", "1104", "1107", "1209", "1211"],
                    )
                });
            let mut traces = BTreeMap::new();
            for levels in source.skill_trees.into_values() {
                let first = levels
                    .get(&1)
                    .ok_or_else(|| error(format!("角色 {id} 的行迹缺少第一级")))?;
                if traces.insert(first.point_id.to_string(), levels).is_some() {
                    return Err(error(format!("角色 {id} 的行迹节点重复")));
                }
            }
            // 推荐资料可能保留旧版行迹。用实际节点确定原始配置的能力版本。
            let ability_version = overrides.ability_versions.get(&id).copied().or_else(|| {
                trace_catalog
                    .as_ref()?
                    .characters
                    .iter()
                    .filter(|binding| binding.character_id == id)
                    .filter(|binding| {
                        binding.skills.values().all(|skill| {
                            traces.get(&skill.node_id).is_some_and(|levels| {
                                levels[&1]
                                    .level_up_skill_id
                                    .iter()
                                    .map(u32::to_string)
                                    .eq(skill.skill_ids.iter().cloned())
                                    && levels
                                        .last_key_value()
                                        .is_some_and(|(&level, _)| level == skill.max_level)
                            })
                        }) && binding.nodes.values().all(|node| traces.contains_key(node))
                    })
                    .max_by_key(|binding| binding.ability_version)
                    .map(|binding| binding.ability_version)
            });
            let recommended: Vec<_> = ids(source.lightcones)
                .into_iter()
                .filter(|cone_id| {
                    light_cones
                        .get(cone_id)
                        .is_some_and(|cone| cone.path == source.base_type)
                })
                .collect();
            let signature = overrides
                .signature_light_cones
                .get(&id)
                .cloned()
                .or_else(|| recommended.first().cloned());
            if rarity == 5
                && !signature
                    .as_ref()
                    .is_some_and(|cone| light_cones.contains_key(cone))
            {
                return Err(error(format!("五星角色 {id} 缺少可用专锥")));
            }
            let teams = source
                .teams
                .into_iter()
                .map(|team| Team {
                    members: ids(team.member_list),
                    backups: [
                        ids(team.backup_list1),
                        ids(team.backup_list2),
                        ids(team.backup_list3),
                    ],
                })
                .collect();
            let relics = Recommendations::from_source(source.relics, &id)?;
            characters.insert(
                id.clone(),
                Profile {
                    id,
                    rarity,
                    path: source.base_type,
                    category,
                    ability_version,
                    signature,
                    recommended,
                    teams,
                    relics,
                    traces,
                },
            );
        }
        Ok(Self {
            game_version: manifest.version,
            data_snapshot: manifest.catalog_snapshot,
            characters,
            light_cones,
        })
    }
}

impl Profile {
    pub fn generate_traces(&self, spec: &CharacterSpec) -> Result<TraceConfig, GenerationError> {
        if let Some(traces) = &spec.traces {
            return Ok(traces.clone());
        }
        let mut result = TraceConfig {
            skill_levels: BTreeMap::new(),
            node_unlocks: BTreeMap::new(),
        };
        let mut levels = BTreeMap::new();
        for (id, source) in &self.traces {
            let level = self.trace_level(id, spec, &mut levels)?;
            if !source[&1].level_up_skill_id.is_empty() {
                result.skill_levels.insert(id.clone(), level);
            } else {
                result.node_unlocks.insert(id.clone(), level > 0);
            }
        }
        Ok(result)
    }

    fn trace_level(
        &self,
        id: &str,
        spec: &CharacterSpec,
        levels: &mut BTreeMap<String, Option<u8>>,
    ) -> Result<u8, GenerationError> {
        if let Some(level) = levels.get(id) {
            return level
                .ok_or_else(|| error(format!("角色 {} 的行迹 {id} 存在循环前置", self.id)));
        }
        let source = self
            .traces
            .get(id)
            .ok_or_else(|| error(format!("角色 {} 缺少行迹 {id}", self.id)))?;
        let first = &source[&1];
        levels.insert(id.to_owned(), None);
        let mut available = true;
        for parent in &first.pre_point {
            available &= self.trace_level(&parent.to_string(), spec, levels)? > 0;
        }
        let level = if available {
            source
                .iter()
                .rev()
                .find(|(_, node)| {
                    spec.level >= node.avatar_level_limit.unwrap_or(1)
                        && spec.promotion >= node.avatar_promotion_limit.unwrap_or(0)
                })
                .map_or(0, |(&level, _)| level)
        } else {
            0
        };
        levels.insert(id.to_owned(), Some(level));
        Ok(level)
    }
}

impl Recommendations {
    fn from_source(source: RelicSource, character_id: &str) -> Result<Self, GenerationError> {
        use crate::relics::{SLOTS, supports_set};
        use account_core::RelicSlot;

        let parse_main = |keys: Vec<String>| {
            keys.iter()
                .map(|key| parse_property(key, character_id))
                .collect::<Result<Vec<RelicMainStat>, _>>()
        };
        let mains = [
            vec![RelicMainStat::Hp],
            vec![RelicMainStat::Atk],
            parse_main(source.property_list3)?,
            parse_main(source.property_list4)?,
            parse_main(source.property_list5)?,
            parse_main(source.property_list6)?,
        ];
        let weights = source
            .sub_affix_property_list
            .iter()
            .enumerate()
            .map(|(i, key)| {
                Ok((
                    parse_property(key, character_id)?,
                    [1.0, 1.0, 0.5, 0.3][i.min(3)],
                ))
            })
            .collect::<Result<_, GenerationError>>()?;
        let outer = ids(source.set4_id_list);
        let planar = ids(source.set2_id_list);
        for (slot, stats) in SLOTS.into_iter().zip(&mains).skip(2) {
            if stats.is_empty() || stats.iter().any(|&stat| !stat.supports_slot(slot)) {
                return Err(error(format!(
                    "角色 {character_id} 的部位 {} 缺少合法的推荐主词条列表",
                    u8::from(slot)
                )));
            }
        }
        for (slot, sets) in [
            (RelicSlot::Head, &outer),
            (RelicSlot::PlanarSphere, &planar),
        ] {
            if sets.is_empty() {
                return Err(error(format!("角色 {character_id} 缺少推荐套装")));
            }
            if let Some(set) = sets.iter().find(|id| !supports_set(slot, id)) {
                return Err(error(format!(
                    "角色 {character_id} 的推荐套装 {set} 与部位不匹配"
                )));
            }
        }
        Ok(Self {
            mains,
            weights,
            outer,
            planar,
        })
    }
}

fn parse_property<T: DeserializeOwned>(
    key: &str,
    character_id: &str,
) -> Result<T, GenerationError> {
    let name = match key {
        "HPDelta" => "HP",
        "AttackDelta" => "ATK",
        "DefenceDelta" => "DEF",
        "HPAddedRatio" => "HP_",
        "AttackAddedRatio" => "ATK_",
        "DefenceAddedRatio" => "DEF_",
        "SpeedDelta" => "SPD",
        "CriticalChanceBase" => "CRIT Rate_",
        "CriticalDamageBase" => "CRIT DMG_",
        "StatusProbabilityBase" => "Effect Hit Rate_",
        "StatusResistanceBase" => "Effect RES_",
        "BreakDamageAddedRatioBase" => "Break Effect_",
        "HealRatioBase" => "Outgoing Healing Boost_",
        "SPRatioBase" => "Energy Regeneration Rate_",
        "PhysicalAddedRatio" => "Physical DMG Boost_",
        "FireAddedRatio" => "Fire DMG Boost_",
        "IceAddedRatio" => "Ice DMG Boost_",
        "ThunderAddedRatio" => "Lightning DMG Boost_",
        "WindAddedRatio" => "Wind DMG Boost_",
        "QuantumAddedRatio" => "Quantum DMG Boost_",
        "ImaginaryAddedRatio" => "Imaginary DMG Boost_",
        _ => return Err(error(format!("角色 {character_id} 的配装属性 {key} 未知"))),
    };
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(name))
        .map_err(|cause| error(format!("角色 {character_id} 的配装属性 {key}: {cause}")))
}

fn ids(values: Vec<u32>) -> Vec<String> {
    values.iter().map(u32::to_string).collect()
}

fn rarity(value: &str) -> Result<u8, GenerationError> {
    value
        .chars()
        .last()
        .and_then(|last| last.to_digit(10))
        .filter(|&n| (3..=5).contains(&n))
        .map(|n| n as u8)
        .ok_or_else(|| error(format!("无效的稀有度 {value}")))
}

fn category(rarity: u8, id: &str, free_prefix: &str, standard: &[&str]) -> GoldCategory {
    if rarity != 5 {
        GoldCategory::None
    } else if id.starts_with(free_prefix) {
        GoldCategory::Free
    } else if standard.contains(&id) {
        GoldCategory::Standard
    } else {
        GoldCategory::Limited
    }
}

fn parse<T: DeserializeOwned>(source: &str, label: &str) -> Result<T, GenerationError> {
    serde_json::from_str(source).map_err(|cause| error(format!("{label}: {cause}")))
}

fn read(path: &Path) -> Result<String, GenerationError> {
    fs::read_to_string(path).map_err(|cause| error(format!("{}: {cause}", path.display())))
}

fn error(message: impl Into<String>) -> GenerationError {
    GenerationError::Reference(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn loading_rejects_invalid_recommendations_before_random_selection() {
        let original: Value =
            serde_json::from_str(include_str!("../data/character/1310.json")).unwrap();
        for (field, candidates, message) in [
            ("set4_id_list", json!([101, 999]), "1310 的推荐套装 999"),
            ("set2_id_list", json!([301, 101]), "1310 的推荐套装 101"),
            (
                "property_list3",
                json!(["AttackAddedRatio", "SpeedDelta"]),
                "1310 的部位 3",
            ),
            (
                "property_list3",
                json!(["UnknownStat"]),
                "1310 的配装属性 UnknownStat",
            ),
        ] {
            let mut profile = original.clone();
            profile["relics"][field] = candidates;
            let result = AccountReference::load(
                include_str!("../data/source-manifest.json"),
                r#"{"1310":null}"#,
                include_str!("../data/lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                &ReferenceOverrides::default(),
            );
            assert!(
                matches!(result, Err(GenerationError::Reference(text)) if text.contains(message))
            );
        }
    }
}
