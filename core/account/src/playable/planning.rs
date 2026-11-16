//! 先确定本体、试用来源和各部分映射选择，再展开完整实际游玩配置。
//!
//! 此阶段实现来源选择和遗器整圈映射判断。其余尚未明确的部分标为待判断，
//! 计划供后续转换器使用，等级、行迹和面板在展开阶段确定。

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    CharacterConfig, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance, RelicSlot,
    playable::{CharacterOrigin, ConfigurationMode},
    reference::{CharacterKind, MappingPart, PlayableData},
};

/// 每部分的映射选择，待判断状态明确保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingAction {
    /// 保留账号本体的对应部分。
    KeepOwned,
    /// 使用该部分的映射模板。
    UseTemplate,
    /// 触发条件尚未确认或尚未计算。
    Undetermined,
}

/// 生成计划所需的模式、已确认 ID 对照和映射选择。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    /// 本体加试用，或全试用。
    pub mode: ConfigurationMode,
    /// 原始角色 ID 到文档角色 ID 的直接对照；由调用方提供已核对关系。
    pub character_aliases: BTreeMap<String, String>,
    /// 按文档角色 ID 提供尚未自动确定的选择；与自动遗器判断冲突时返回错误。
    pub mapping_decisions: BTreeMap<String, BTreeMap<MappingPart, MappingAction>>,
}

/// 一种角色后续展开所采用的来源与映射选择。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCharacter {
    /// 文档角色 ID。
    pub character_id: String,
    /// 本体、试用或玩法专属来源。
    pub origin: CharacterOrigin,
    /// 本体的六部分选择；试用和玩法专属角色为空。
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

/// 按持有情况补齐试用来源，并根据遗器等级判断内外圈整体映射。
///
/// 仅按角色 ID 及调用方已核对的直接别名查找持有情况。两份原始角色映射到
/// 同一目标时返回冲突。已知遗器未满级时采用整圈模板；全部满级时保留原装。
/// 空缺部位使“全部满级”尚无法确认，保留待判断。完整配置和面板由后续阶段展开。
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
    for character_id in request.mapping_decisions.keys() {
        if request.mode == ConfigurationMode::TrialsOnly
            || !owned.contains_key(character_id.as_str())
        {
            return Err(PlanError(format!(
                "角色 {character_id} 当前采用试用来源，无法应用本体映射选择"
            )));
        }
        if !data
            .mappings
            .iter()
            .any(|entry| &entry.character_id == character_id)
        {
            return Err(PlanError(format!("角色 {character_id} 没有映射模板")));
        }
    }
    let parts = [
        MappingPart::CharacterProgression,
        MappingPart::Eidolons,
        MappingPart::Traces,
        MappingPart::LightCone,
        MappingPart::OuterRelics,
        MappingPart::PlanarRelics,
    ];
    let mut relics = BTreeMap::new();
    for relic in &raw.relics {
        if relics.insert(relic.instance_id.as_str(), relic).is_some() {
            return Err(PlanError(format!("遗器实例 ID {} 重复", relic.instance_id)));
        }
    }
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
                            data.rules.relic_mapping_minimum_level,
                        )?,
                        MappingPart::PlanarRelics => relic_mapping_action(
                            character,
                            &relics,
                            &[RelicSlot::PlanarSphere, RelicSlot::LinkRope],
                            data.rules.relic_mapping_minimum_level,
                        )?,
                        _ => MappingAction::Undetermined,
                    };
                    let specified = request
                        .mapping_decisions
                        .get(&trial.character_id)
                        .and_then(|choices| choices.get(&part))
                        .copied();
                    let action = match specified {
                        Some(choice) if automatic == MappingAction::Undetermined => choice,
                        Some(choice)
                            if choice != MappingAction::Undetermined && choice != automatic =>
                        {
                            return Err(PlanError(format!(
                                "角色 {} 的 {part:?} 选择与遗器等级规则冲突",
                                trial.character_id
                            )));
                        }
                        _ => automatic,
                    };
                    Ok((part, action))
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
    minimum_level: u8,
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
        if relic.slot != *slot {
            return Err(PlanError(format!(
                "角色 {} 的遗器 {instance_id} 部位不一致",
                character.character_id
            )));
        }
        if relic.level > minimum_level {
            return Err(PlanError(format!(
                "遗器 {instance_id} 等级超过资料上限 {minimum_level}"
            )));
        }
        has_underleveled |= relic.level < minimum_level;
    }
    Ok(if has_underleveled {
        MappingAction::UseTemplate
    } else if has_empty_slot {
        MappingAction::Undetermined
    } else {
        MappingAction::KeepOwned
    })
}
