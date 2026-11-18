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

type TraceLevels = BTreeMap<u8, TraceLevel>;

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
    traces: Vec<(String, TraceLevels)>,
}

#[derive(Debug)]
pub(crate) struct Cone {
    rarity: u8,
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
            let category = category(
                rarity,
                &id,
                "24",
                &[
                    "23000", "23002", "23003", "23004", "23005", "23012", "23013",
                ],
                overrides.light_cone_categories.get(&id).copied(),
            )?;
            light_cones.insert(
                id.clone(),
                Cone {
                    rarity,
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
            let category = category(
                rarity,
                &id,
                "80",
                &["1003", "1004", "1101", "1104", "1107", "1209", "1211"],
                overrides.character_categories.get(&id).copied(),
            )?;
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
                .or_else(|| {
                    recommended
                        .iter()
                        .find(|id| light_cones[*id].rarity == 5)
                        .cloned()
                });
            if rarity == 5
                && !signature
                    .as_ref()
                    .and_then(|id| light_cones.get(id))
                    .is_some_and(|cone| cone.rarity == 5 && cone.path == source.base_type)
            {
                return Err(error(format!(
                    "五星角色 {id} 需要同命途五星专锥，当前为 {signature:?}"
                )));
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
            let traces = ordered_traces(traces, &id)?;
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

/// 按前置关系排列节点，生成时可以直接读取已算好的等级。
fn ordered_traces(
    mut nodes: BTreeMap<String, BTreeMap<u8, TraceLevel>>,
    character_id: &str,
) -> Result<Vec<(String, TraceLevels)>, GenerationError> {
    for (id, levels) in &nodes {
        for parent in &levels[&1].pre_point {
            if !nodes.contains_key(&parent.to_string()) {
                return Err(error(format!(
                    "角色 {character_id} 的行迹 {id} 缺少前置 {parent}"
                )));
            }
        }
    }
    let mut ordered = Vec::with_capacity(nodes.len());
    while !nodes.is_empty() {
        let mut ready = Vec::new();
        for (id, levels) in &nodes {
            if levels[&1]
                .pre_point
                .iter()
                .all(|parent| !nodes.contains_key(&parent.to_string()))
            {
                ready.push(id.clone());
            }
        }
        if ready.is_empty() {
            let (id, levels) = nodes.first_key_value().expect("pending trace");
            return Err(error(format!(
                "角色 {character_id} 的行迹 {id} 依赖循环前置 {:?}",
                levels[&1].pre_point
            )));
        }
        for id in ready {
            let levels = nodes.remove(&id).expect("ready trace");
            ordered.push((id, levels));
        }
    }
    Ok(ordered)
}

impl Profile {
    pub fn generate_traces(&self, spec: &CharacterSpec) -> TraceConfig {
        if let Some(traces) = &spec.traces {
            return traces.clone();
        }
        let mut result = TraceConfig {
            skill_levels: BTreeMap::new(),
            node_unlocks: BTreeMap::new(),
        };
        let mut levels = BTreeMap::new();
        for (id, source) in &self.traces {
            let first = &source[&1];
            let level = if first.pre_point.iter().all(|parent| levels[parent] > 0) {
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
            levels.insert(first.point_id, level);
            if !first.level_up_skill_id.is_empty() {
                result.skill_levels.insert(id.clone(), level);
            } else {
                result.node_unlocks.insert(id.clone(), level > 0);
            }
        }
        result
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

fn category(
    rarity: u8,
    id: &str,
    free_prefix: &str,
    standard: &[&str],
    override_value: Option<GoldCategory>,
) -> Result<GoldCategory, GenerationError> {
    if let Some(value) = override_value {
        if rarity != 5 && value != GoldCategory::None {
            return Err(error(format!("非五星条目 {id} 不能计入五星投入")));
        }
        return Ok(value);
    }
    Ok(if rarity != 5 {
        GoldCategory::None
    } else if id.starts_with(free_prefix) {
        GoldCategory::Free
    } else if standard.contains(&id) {
        GoldCategory::Standard
    } else {
        GoldCategory::Limited
    })
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
    use crate::{AccountRequest, ChaCha8Rng, SeedableRng, generate_account};
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

    #[test]
    fn edited_references_reject_inconsistent_categories_signatures_and_prerequisites() {
        let original: Value =
            serde_json::from_str(include_str!("../data/character/1310.json")).unwrap();
        for (profile, overrides, expected) in [
            (
                original.clone(),
                ReferenceOverrides {
                    light_cone_categories: BTreeMap::from([(
                        "20000".into(),
                        GoldCategory::Limited,
                    )]),
                    ..Default::default()
                },
                "20000",
            ),
            (
                original.clone(),
                ReferenceOverrides {
                    signature_light_cones: BTreeMap::from([("1310".into(), "20000".into())]),
                    ..Default::default()
                },
                "20000",
            ),
            (
                {
                    let mut source = original.clone();
                    source["rarity"] = json!("CombatPowerAvatarRarityType4");
                    source
                },
                ReferenceOverrides {
                    character_categories: BTreeMap::from([("1310".into(), GoldCategory::Standard)]),
                    ..Default::default()
                },
                "1310",
            ),
            (
                {
                    let mut source = original.clone();
                    source["skill_trees"]["1310001"]["1"]["pre_point"] = json!([1310001]);
                    source
                },
                Default::default(),
                "1310001",
            ),
            (
                {
                    let mut source = original;
                    source["skill_trees"]["1310001"]["1"]["pre_point"] = json!([999999]);
                    source
                },
                Default::default(),
                "999999",
            ),
        ] {
            let loaded = AccountReference::load(
                include_str!("../data/source-manifest.json"),
                r#"{"1310":null}"#,
                include_str!("../data/lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                &overrides,
            );
            assert!(
                matches!(loaded, Err(GenerationError::Reference(ref text)) if text.contains(expected)),
                "{expected}"
            );
        }
    }

    #[test]
    fn uniform_welt_build_uses_a_five_star_cone_when_a_four_star_is_recommended_first() {
        let mut profile: Value =
            serde_json::from_str(include_str!("../data/character/1004.json")).unwrap();
        profile["lightcones"] = json!([21001, 23004]);
        let load = |overrides: &ReferenceOverrides| {
            AccountReference::load(
                include_str!("../data/source-manifest.json"),
                r#"{"1004":null}"#,
                include_str!("../data/lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                overrides,
            )
        };
        let request = AccountRequest {
            five_star_build: Some((0, 1)),
            ..AccountRequest::new("welt", 1)
        };
        let reference = load(&ReferenceOverrides::default()).unwrap();
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        assert_eq!(result.account.light_cones[0].light_cone_id, "23004");
        assert_eq!(result.gold_count(), 1.0);
        let overrides = ReferenceOverrides {
            signature_light_cones: BTreeMap::from([("1004".into(), "21001".into())]),
            ..Default::default()
        };
        assert!(
            matches!(load(&overrides), Err(GenerationError::Reference(text)) if text.contains("21001"))
        );
    }

    #[test]
    fn external_snapshot_retains_its_id_and_can_provide_a_missing_signature() {
        let mut profile: Value =
            serde_json::from_str(include_str!("../data/character/1004.json")).unwrap();
        profile["lightcones"] = json!([]);
        let load = |overrides: &ReferenceOverrides| {
            AccountReference::load(
                r#"{"version":"4.7","catalog_snapshot":"test-snapshot"}"#,
                r#"{"1004":null}"#,
                include_str!("../data/lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                overrides,
            )
        };
        assert!(
            matches!(load(&ReferenceOverrides::default()), Err(GenerationError::Reference(message)) if message.contains("1004"))
        );
        let reference = load(&ReferenceOverrides {
            signature_light_cones: BTreeMap::from([("1004".into(), "23004".into())]),
            ..ReferenceOverrides::default()
        })
        .unwrap();
        let request = AccountRequest {
            five_star_build: Some((0, 1)),
            ..AccountRequest::new("snapshot", 1)
        };
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        assert_eq!(
            result.account.data_snapshot.as_deref(),
            Some("test-snapshot")
        );
        assert_eq!(result.account.light_cones[0].light_cone_id, "23004");
    }
}
