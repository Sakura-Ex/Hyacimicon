//! 先确定本体、试用来源和各部分映射选择，再展开完整实际游玩配置。
//!
//! 来源选择和遗器整圈判断由计划入口统一执行，完整转换直接复用其结果。

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    CharacterConfig, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance, RelicSlot,
    playable::{CharacterOrigin, ConfigurationMode, RELIC_MAPPING_LEVEL},
    reference::{CharacterData, CharacterKind, MappingPart, PlayableData},
};

/// 一圈遗器保留原装或采用映射模板。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingAction {
    /// 保留账号本体的对应部分。
    KeepOwned,
    /// 使用该部分的映射模板。
    UseTemplate,
}

/// 生成计划所需的资源模式和已确认 ID 对照。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    /// 本体加试用，或全试用。
    pub mode: ConfigurationMode,
    /// 原始角色 ID 到文档角色 ID 的直接对照；由调用方提供已核对关系。
    pub character_aliases: BTreeMap<String, String>,
}

/// 一种角色后续展开所采用的来源与映射选择。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCharacter {
    /// 文档角色 ID。
    pub character_id: String,
    /// 本体、试用或玩法专属来源。
    pub origin: CharacterOrigin,
    /// 本体的分项选择；试用和玩法专属角色为空。
    pub mapping_decisions: BTreeMap<MappingPart, MappingAction>,
}

/// 来源选择与分项映射计划，供后续配置展开和缺项检查。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationPlan {
    /// 原始配置标识。
    pub raw_config_id: String,
    /// 本次选择的数据快照。
    pub data_snapshot: String,
    /// 资源模式。
    pub mode: ConfigurationMode,
    /// 按试用表顺序覆盖全部玩法角色。
    pub characters: Vec<PlannedCharacter>,
    /// 当前资料范围之外的持有角色 ID，保留供调用方查看。
    pub unused_raw_character_ids: Vec<String>,
}

/// 版本、角色标识或计划输入存在冲突。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError(
    /// 具体冲突原因。
    pub String,
);

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PlanError {}

/// 按角色稀有度确定星魂映射目标：四星为 6，五星为 0。
///
/// 依据常规稀有度计算；缺少四／五星稀有度时返回具体错误。
pub fn mapped_eidolons(character: &CharacterData) -> Result<u8, PlanError> {
    match character.rarity {
        Some(4) => Ok(6),
        Some(5) => Ok(0),
        _ => Err(PlanError(format!(
            "角色 {} 缺少适用于星魂映射的四／五星稀有度",
            character.id
        ))),
    }
}

/// 按持有情况补齐试用来源，并根据遗器等级判断内外圈整体映射。
///
/// 仅按角色 ID 及调用方已核对的直接别名查找持有情况。两份原始角色映射到
/// 同一目标时返回冲突。已知遗器未满级时采用整圈模板；全部满级时保留原装。
/// 空缺部位同样触发所在圈映射。完整配置和面板由转换入口展开。
pub fn plan_playable_account(
    raw: &RawAccount,
    data: &PlayableData,
    request: &PlanRequest,
) -> Result<ConfigurationPlan, PlanError> {
    if raw.schema_version != RAW_ACCOUNT_SCHEMA_VERSION {
        return Err(PlanError("原始账号格式版本不受支持".to_owned()));
    }
    if let Some(version) = &raw.game_version
        && version != &data.manifest.game_version
    {
        return Err(PlanError("原始账号与目标资料的游戏版本不一致".to_owned()));
    }
    let target_ids: BTreeSet<_> = data
        .characters
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    for target in request.character_aliases.values() {
        if !target_ids.contains(target.as_str()) {
            return Err(PlanError(format!("别名目标 {target} 未包含在资料中")));
        }
    }
    let mut owned = BTreeMap::new();
    let mut unused = Vec::new();
    let mut raw_ids = BTreeSet::new();
    for character in &raw.characters {
        if !raw_ids.insert(&character.character_id) {
            return Err(PlanError(format!(
                "原始角色 ID {} 重复",
                character.character_id
            )));
        }
        let target = request
            .character_aliases
            .get(&character.character_id)
            .unwrap_or(&character.character_id);
        if !target_ids.contains(target.as_str()) {
            unused.push(character.character_id.clone());
            continue;
        }
        if owned.insert(target.as_str(), character).is_some() {
            return Err(PlanError(format!(
                "多个持有条目对应角色 {target}，需要先确定形态关系"
            )));
        }
    }
    let parts = [MappingPart::OuterRelics, MappingPart::PlanarRelics];
    let mut relics = BTreeMap::new();
    for relic in &raw.relics {
        if relics.insert(relic.instance_id.as_str(), relic).is_some() {
            return Err(PlanError(format!("遗器实例 ID {} 重复", relic.instance_id)));
        }
    }
    validate_equipment(raw)?;
    let mut characters = Vec::with_capacity(data.trials.len());
    for trial in &data.trials {
        let definition = data
            .characters
            .iter()
            .find(|entry| entry.id == trial.character_id)
            .ok_or_else(|| PlanError(format!("试用模板 {} 没有角色资料", trial.character_id)))?;
        let (origin, mapping_decisions) = if definition.kind == CharacterKind::GameplayExclusive {
            if owned.contains_key(trial.character_id.as_str()) {
                return Err(PlanError(format!(
                    "玩法专属角色 {} 应从模板取得",
                    trial.character_id
                )));
            }
            (
                CharacterOrigin::GameplayExclusive {
                    template_id: trial.character_id.clone(),
                },
                BTreeMap::new(),
            )
        } else if request.mode == ConfigurationMode::OwnedWithTrials
            && let Some(character) = owned.get(trial.character_id.as_str())
        {
            let choices = parts
                .into_iter()
                .map(|part| {
                    let automatic = match part {
                        MappingPart::OuterRelics => relic_mapping_action(
                            character,
                            &relics,
                            &[
                                RelicSlot::Head,
                                RelicSlot::Hands,
                                RelicSlot::Body,
                                RelicSlot::Feet,
                            ],
                        )?,
                        MappingPart::PlanarRelics => relic_mapping_action(
                            character,
                            &relics,
                            &[RelicSlot::PlanarSphere, RelicSlot::LinkRope],
                        )?,
                    };
                    Ok((part, automatic))
                })
                .collect::<Result<_, PlanError>>()?;
            (
                CharacterOrigin::Owned {
                    raw_character_id: character.character_id.clone(),
                },
                choices,
            )
        } else {
            (
                CharacterOrigin::Trial {
                    template_id: trial.character_id.clone(),
                },
                BTreeMap::new(),
            )
        };
        characters.push(PlannedCharacter {
            character_id: trial.character_id.clone(),
            origin,
            mapping_decisions,
        });
    }
    Ok(ConfigurationPlan {
        raw_config_id: raw.config_id.clone(),
        data_snapshot: data.manifest.snapshot_id.clone(),
        mode: request.mode,
        characters,
        unused_raw_character_ids: unused,
    })
}

/// 检查一圈全部装备引用；任意已装备遗器未满级即整圈映射。
fn relic_mapping_action(
    character: &CharacterConfig,
    relics: &BTreeMap<&str, &RelicInstance>,
    slots: &[RelicSlot],
) -> Result<MappingAction, PlanError> {
    let mut has_underleveled = false;
    let mut has_empty_slot = false;
    for slot in slots {
        let Some(instance_id) = character.relic_instance_ids.get(slot) else {
            has_empty_slot = true;
            continue;
        };
        let relic = relics.get(instance_id.as_str()).ok_or_else(|| {
            PlanError(format!(
                "角色 {} 引用的遗器 {instance_id} 不存在",
                character.character_id
            ))
        })?;
        has_underleveled |= relic.level < RELIC_MAPPING_LEVEL;
    }
    Ok(if has_underleveled || has_empty_slot {
        MappingAction::UseTemplate
    } else {
        MappingAction::KeepOwned
    })
}

/// 核对库存光锥 ID 及跨角色的装备归属，避免同一实例同时装备。
fn validate_equipment(raw: &RawAccount) -> Result<(), PlanError> {
    let mut cone_ids = BTreeSet::new();
    for cone in &raw.light_cones {
        if !cone_ids.insert(&cone.instance_id) {
            return Err(PlanError(format!("光锥实例 ID {} 重复", cone.instance_id)));
        }
        if !(1..=5).contains(&cone.superimposition) {
            return Err(PlanError(format!(
                "光锥 {} 的叠影超出 1～5",
                cone.light_cone_id
            )));
        }
    }
    for relic in &raw.relics {
        let max_level = RelicInstance::max_level_for_rarity(relic.rarity)
            .ok_or_else(|| PlanError(format!("遗器 {} 的品质超出范围", relic.instance_id)))?;
        if relic.level > max_level {
            return Err(PlanError(format!(
                "遗器 {} 的品质或强化等级超出范围",
                relic.instance_id
            )));
        }
        if !relic.mainstat.supports_slot(relic.slot) {
            return Err(PlanError(format!(
                "遗器 {} 的主词条与部位不匹配",
                relic.instance_id
            )));
        }
        if relic.substats.len() > RelicInstance::MAX_SUBSTATS {
            return Err(PlanError(format!(
                "遗器 {} 的副词条数量超出范围",
                relic.instance_id
            )));
        }
        let main: super::attributes::Attribute = relic.mainstat.into();
        if relic
            .substats
            .keys()
            .any(|key| super::attributes::Attribute::from(*key) == main)
        {
            return Err(PlanError(format!(
                "遗器 {} 的主副词条重复",
                relic.instance_id
            )));
        }
        if relic
            .substats
            .values()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(PlanError(format!(
                "遗器 {} 的副词条应为有限正数",
                relic.instance_id
            )));
        }
    }
    let mut assigned_cones = BTreeSet::new();
    let mut assigned_relics = BTreeSet::new();
    for character in &raw.characters {
        if let Some(id) = &character.light_cone_instance_id {
            if !cone_ids.contains(id) {
                return Err(PlanError(format!(
                    "角色 {} 引用的光锥 {id} 不存在",
                    character.character_id
                )));
            }
            if !assigned_cones.insert(id) {
                return Err(PlanError(format!("光锥实例 {id} 被多个角色装备")));
            }
        }
        for (slot, id) in &character.relic_instance_ids {
            let relic = raw
                .relics
                .iter()
                .find(|entry| &entry.instance_id == id)
                .ok_or_else(|| {
                    PlanError(format!(
                        "角色 {} 引用的遗器 {id} 不存在",
                        character.character_id
                    ))
                })?;
            if relic.slot != *slot {
                return Err(PlanError(format!(
                    "角色 {} 的遗器 {id} 部位不一致",
                    character.character_id
                )));
            }
            if !assigned_relics.insert(id) {
                return Err(PlanError(format!("遗器实例 {id} 被多个角色装备")));
            }
        }
    }
    Ok(())
}
