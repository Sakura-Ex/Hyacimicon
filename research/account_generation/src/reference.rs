use std::{collections::BTreeMap, fs, path::Path};

use account_core::{RelicMainStat, RelicSubStat, TraceConfig, reference::traces::TraceCatalog};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{CharacterSpec, GenerationError, GoldCategory};

/// 可复用的同版本生成资料，包含角色、光锥、推荐配装和行迹条件。
///
/// 加载时校验推荐主词条、套装、五星专锥及一级行迹前置关系。可在多次生成间共享同一份资料。
#[derive(Debug)]
pub struct AccountReference {
    /// 此资料使用的游戏版本。
    pub(crate) game_version: String,
    /// 上游角色图鉴快照 ID；未提供时为空。
    pub(crate) data_snapshot: Option<String>,
    /// 按角色 ID 索引的已解析生成资料。
    pub(crate) characters: BTreeMap<String, Profile>,
    /// 按光锥 ID 索引的已解析生成资料。
    pub(crate) light_cones: BTreeMap<String, Cone>,
}

/// 同一份资料中显式指定的金数类别、专锥及能力版本。
///
/// 键为角色或光锥 ID。五星角色的 `signature_light_cones` 必须指向同命途五星光锥；
/// `ability_versions` 用于指定原始行迹所对应的能力版本。
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReferenceOverrides {
    /// 覆盖角色的五星金数类别。
    pub character_categories: BTreeMap<String, GoldCategory>,
    /// 覆盖光锥的五星金数类别。
    pub light_cone_categories: BTreeMap<String, GoldCategory>,
    /// 指定五星角色采用的同命途五星专锥。
    pub signature_light_cones: BTreeMap<String, String>,
    /// 覆盖角色的原始能力版本号。
    pub ability_versions: BTreeMap<String, u32>,
}

/// 同一角色行迹节点各等级的资料映射。
type TraceLevels = BTreeMap<u8, TraceLevel>;

/// 供角色筛选和账号生成使用的已解析角色资料。
#[derive(Debug)]
pub(crate) struct Profile {
    /// 角色 ID。
    pub id: String,
    /// 角色稀有度。
    pub rarity: u8,
    /// 角色命途，用于筛选自动配装的光锥。
    pub path: String,
    /// 金数预算类别。
    pub category: GoldCategory,
    /// 显式覆盖的能力版本，或与角色行迹匹配的最高 4.7 绑定版本。无匹配时为空。
    pub ability_version: Option<u32>,
    /// 专锥覆盖指定的 ID，未指定时取推荐列表中的首个同命途五星光锥。
    pub signature: Option<String>,
    /// 按资料顺序排列且与角色命途匹配的推荐光锥 ID。
    pub recommended: Vec<String>,
    /// 推荐队伍及其替补资料。
    pub teams: Vec<Team>,
    /// 推荐遗器主词条、副词条权重及套装。
    pub relics: Recommendations,
    /// 按前置拓扑顺序排列的行迹节点和各级数据。
    traces: Vec<(String, TraceLevels)>,
}

/// 供配装搜索使用的已解析光锥资料。
#[derive(Debug)]
pub(crate) struct Cone {
    /// 光锥稀有度。
    rarity: u8,
    /// 光锥 ID。
    pub id: String,
    /// 光锥命途。
    pub path: String,
    /// 金数预算类别。
    pub category: GoldCategory,
    /// 是否限制为单个持有实例。
    pub unique: bool,
}

/// 解析后的主词条、副词条权重和套装推荐。
#[derive(Debug)]
pub(crate) struct Recommendations {
    /// 按遗器部位顺序排列的推荐主词条。
    pub mains: [Vec<RelicMainStat>; 6],
    /// 推荐副词条的有效词条评分权重。
    pub weights: BTreeMap<RelicSubStat, f64>,
    /// 四件套推荐 ID。
    pub outer: Vec<String>,
    /// 两件套推荐 ID。
    pub planar: Vec<String>,
}

/// 推荐队伍中的首选成员及各位置替补。
#[derive(Debug)]
pub(crate) struct Team {
    /// 每个位置的首选角色 ID。
    pub members: Vec<String>,
    /// 与首选位置一一对应的替补 ID 列表。
    pub backups: [Vec<String>; 3],
}

/// 外部资料清单中的游戏版本和图鉴快照标识。
#[derive(Deserialize)]
struct Manifest {
    /// 资料对应的游戏版本字符串。
    version: String,
    /// 生成账号需保留的图鉴快照 ID。
    catalog_snapshot: Option<String>,
}

/// 上游光锥 JSON 中生成所需的字段。
#[derive(Deserialize)]
struct ConeSource {
    /// 上游稀有度枚举名称。
    rank: String,
    /// 光锥命途字段。
    #[serde(rename = "baseType")]
    path: String,
}

/// 上游角色 JSON 中生成所需的字段。
#[derive(Deserialize)]
struct ProfileSource {
    /// 上游稀有度枚举名称。
    rarity: String,
    /// 角色命途字段。
    base_type: String,
    /// 推荐光锥的数字 ID 列表。
    lightcones: Vec<u32>,
    /// 推荐遗器主词条、副词条和套装资料。
    relics: RelicSource,
    /// 推荐队伍及替补资料。
    teams: Vec<TeamSource>,
    /// 以技能树节点为键的行迹等级资料。
    skill_trees: BTreeMap<String, BTreeMap<u8, TraceLevel>>,
}

/// 上游角色遗器推荐字段。
#[derive(Deserialize)]
struct RelicSource {
    /// 躯干主词条候选。
    property_list3: Vec<String>,
    /// 脚部主词条候选。
    property_list4: Vec<String>,
    /// 位面球主词条候选。
    property_list5: Vec<String>,
    /// 连结绳主词条候选。
    property_list6: Vec<String>,
    /// 按推荐优先级排列的副词条属性。
    sub_affix_property_list: Vec<String>,
    /// 四件套推荐数字 ID。
    set4_id_list: Vec<u32>,
    /// 两件套推荐数字 ID。
    set2_id_list: Vec<u32>,
}

/// 上游推荐队伍的三个队友位置，以及各位置可用的替补。
#[derive(Deserialize)]
struct TeamSource {
    /// 首选角色数字 ID 列表。
    member_list: Vec<u32>,
    /// 第二个队员位置的替补角色 ID 列表。
    backup_list1: Vec<u32>,
    /// 第三个队员位置的替补角色 ID 列表。
    backup_list2: Vec<u32>,
    /// 第四个队员位置的替补角色 ID 列表。
    backup_list3: Vec<u32>,
}

/// 一个行迹节点某一级的解锁条件与关联技能。
#[derive(Debug, Deserialize)]
struct TraceLevel {
    /// 行迹节点数字 ID。
    point_id: u32,
    /// 解锁该级所需的最低角色等级。
    avatar_level_limit: Option<u8>,
    /// 解锁该级所需的最低角色突破阶段。
    avatar_promotion_limit: Option<u8>,
    /// 节点关联的技能 ID。一级记录有技能时输出技能等级，否则输出节点的解锁状态。
    level_up_skill_id: Vec<u32>,
    /// 一级记录列出解锁本节点所需的前置节点 ID。后续等级沿用该列表。
    pre_point: Vec<u32>,
}

/// 创建外部的生成参考资料。
impl AccountReference {
    /// 加载外部资料目录。调用方可提供配套行迹资料以识别能力版本。
    ///
    /// 目录需包含 `source-manifest.json`、`character.json`、`character/<角色 ID>.json`
    /// 和 `lightcone.json`。清单版本决定游戏版本标记；清单中的 `catalog_snapshot`
    /// 会保留到生成的原始账号。角色资料须覆盖目录列出的角色，并为每个行迹节点
    /// 提供一级 `pre_point`；缺少前置节点或存在循环依赖时加载失败。
    pub fn from_dir(
        path: &Path,
        trace_catalog: Option<&TraceCatalog>,
        overrides: &ReferenceOverrides,
    ) -> Result<Self, GenerationError> {
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
            trace_catalog,
            overrides,
        )
    }

    /// 解析清单和基础索引、加载角色资料并执行跨条目校验。
    fn load(
        manifest: &str,
        characters: &str,
        cones: &str,
        profile: impl Fn(&str) -> Result<ProfileSource, GenerationError>,
        trace_catalog: Option<&TraceCatalog>,
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
                trace_catalog?
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

/// 按依赖拓扑排列节点；缺失父节点或循环依赖时返回资料错误。
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

/// 从角色资料生成显式或按练度解锁的行迹配置。
impl Profile {
    /// 有显式行迹配置时原样返回，否则按角色等级、突破和前置条件生成。
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

/// 将上游推荐字段转换并校验为生成器使用的遗器条件。
impl Recommendations {
    /// 解析主词条、副词条权重及套装推荐，并检查各部位的合法性。
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

/// 将上游属性名转换为 account-core 的遗器主词条或副词条类型。
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

/// 将上游数字 ID 列表转换为十进制字符串列表。
fn ids(values: Vec<u32>) -> Vec<String> {
    values.iter().map(u32::to_string).collect()
}

/// 从稀有度名称末尾提取 3 至 5 的星级数值。
fn rarity(value: &str) -> Result<u8, GenerationError> {
    value
        .chars()
        .last()
        .and_then(|last| last.to_digit(10))
        .filter(|&n| (3..=5).contains(&n))
        .map(|n| n as u8)
        .ok_or_else(|| error(format!("无效的稀有度 {value}")))
}

/// 应用显式或默认金数类别，并拒绝将非五星条目计入五星投入。
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

/// 反序列化 JSON 文本，并在错误消息中附加资料标签。
fn parse<T: DeserializeOwned>(source: &str, label: &str) -> Result<T, GenerationError> {
    serde_json::from_str(source).map_err(|cause| error(format!("{label}: {cause}")))
}

/// 读取 UTF-8 资料文件，并在错误中保留文件路径。
fn read(path: &Path) -> Result<String, GenerationError> {
    fs::read_to_string(path).map_err(|cause| error(format!("{}: {cause}", path.display())))
}

/// 包装一条参考资料格式或一致性错误。
fn error(message: impl Into<String>) -> GenerationError {
    GenerationError::Reference(message.into())
}

/// 覆盖参考资料解析、校验和外部快照传递的单元测试。
#[cfg(all(test, feature = "private-data-tests"))]
mod tests {
    use super::*;
    use crate::{AccountRequest, ChaCha8Rng, SeedableRng, generate_account};
    use serde_json::{Value, json};

    /// 验证非法推荐主词条、套装和未知属性会在随机选择前被拒绝。
    #[test]
    fn loading_rejects_invalid_recommendations_before_random_selection() {
        let original: Value = serde_json::from_str(&crate::private_test_data::read_reference(
            "character/1310.json",
        ))
        .unwrap();
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
                &crate::private_test_data::read_reference("source-manifest.json"),
                r#"{"1310":null}"#,
                &crate::private_test_data::read_reference("lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                Some(&crate::private_test_data::trace_catalog()),
                &ReferenceOverrides::default(),
            );
            assert!(
                matches!(result, Err(GenerationError::Reference(text)) if text.contains(message))
            );
        }
    }

    /// 验证分类冲突、无效专锥、缺失前置和循环前置均导致加载失败。
    #[test]
    fn edited_references_reject_inconsistent_categories_signatures_and_prerequisites() {
        let original: Value = serde_json::from_str(&crate::private_test_data::read_reference(
            "character/1310.json",
        ))
        .unwrap();
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
                &crate::private_test_data::read_reference("source-manifest.json"),
                r#"{"1310":null}"#,
                &crate::private_test_data::read_reference("lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                Some(&crate::private_test_data::trace_catalog()),
                &overrides,
            );
            assert!(
                matches!(loaded, Err(GenerationError::Reference(ref text)) if text.contains(expected)),
                "{expected}"
            );
        }
    }

    /// 验证统一五星配装从推荐列表筛选五星光锥并拒绝无效专锥覆盖。
    #[test]
    fn uniform_welt_build_uses_a_five_star_cone_when_a_four_star_is_recommended_first() {
        let mut profile: Value = serde_json::from_str(&crate::private_test_data::read_reference(
            "character/1004.json",
        ))
        .unwrap();
        profile["lightcones"] = json!([21001, 23004]);
        let load = |overrides: &ReferenceOverrides| {
            AccountReference::load(
                &crate::private_test_data::read_reference("source-manifest.json"),
                r#"{"1004":null}"#,
                &crate::private_test_data::read_reference("lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                Some(&crate::private_test_data::trace_catalog()),
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

    /// 验证外部快照标识会写入 RawAccount，且专锥覆盖可补齐缺失推荐。
    #[test]
    fn external_snapshot_retains_its_id_and_can_provide_a_missing_signature() {
        let mut profile: Value = serde_json::from_str(&crate::private_test_data::read_reference(
            "character/1004.json",
        ))
        .unwrap();
        profile["lightcones"] = json!([]);
        let load = |overrides: &ReferenceOverrides| {
            AccountReference::load(
                r#"{"version":"4.7","catalog_snapshot":"test-snapshot"}"#,
                r#"{"1004":null}"#,
                &crate::private_test_data::read_reference("lightcone.json"),
                |_| Ok(serde_json::from_value(profile.clone()).unwrap()),
                Some(&crate::private_test_data::trace_catalog()),
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
