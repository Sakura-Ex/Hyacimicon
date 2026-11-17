//! 将原始账号、试用模板和共用规则展开为完整游玩配置。

use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    CharacterConfig, LightConeInstance, RawAccount, RelicInstance, RelicSlot, TraceConfig,
    reference::{
        CharacterData, CharacterKind, PlayableData, SkillKind, TraceBinding, TrialTemplate,
    },
};

use super::{
    CharacterOrigin, EquipmentOrigin, PLAYABLE_ACCOUNT_SCHEMA_VERSION, PlayableAccount,
    PlayableCharacter, PlayableLightCone, PlayableRelicGroup, PlayableRelics, PlayableTraces,
    TEMPLATE_LIGHT_CONE_SUPERIMPOSITION,
    attributes::AttributeValues,
    panel,
    selection::{mapped_eidolons, owned_characters},
    traces::raise_traces_to_trial,
};

/// 完整转换使用的规则版本。
pub const CONVERSION_RULES_VERSION: &str = "s2-4.7-v2";

/// 完整转换所需的输出标识和角色 ID 对照。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionRequest {
    /// 本次游玩配置的标识。
    pub config_id: String,
    /// 已核对的原始角色 ID 到模板角色 ID 对照。
    pub character_aliases: BTreeMap<String, String>,
}

/// 账号版本、资源引用或静态配置转换中的具体错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError(
    /// 具体错误原因。
    pub String,
);

impl fmt::Display for ConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ConversionError {}

/// 展开全部玩法角色的行迹、光锥、遗器和静态面板，保留原始账号。
pub fn convert_playable_account(
    raw: &RawAccount,
    data: &PlayableData,
    request: &ConversionRequest,
) -> Result<PlayableAccount, ConversionError> {
    if request.config_id.trim().is_empty() {
        return Err(ConversionError("游玩配置标识不能为空".to_owned()));
    }
    let owned_index = owned_characters(raw, data, &request.character_aliases)?;
    let mut characters = Vec::with_capacity(data.trials.len());
    for trial in &data.trials {
        let id = &trial.character_id;
        let definition = data
            .characters
            .iter()
            .find(|entry| &entry.id == id)
            .ok_or_else(|| ConversionError(format!("缺少角色 {id} 的基础数据")))?;
        let owned = owned_index.get(id).copied();
        let eidolons = match owned {
            Some(character) => character.eidolons.max(mapped_eidolons(definition)?),
            None => trial.eidolons,
        };
        if eidolons > 6 {
            return Err(ConversionError(format!("角色 {id} 的星魂超出 0～6")));
        }
        let binding = data.traces.latest(id);
        if binding.is_none() && definition.kind != CharacterKind::GameplayExclusive {
            return Err(ConversionError(format!("缺少角色 {id} 的行迹绑定")));
        }
        let traces = match binding {
            Some(binding) => expand_traces(owned, trial, binding, eidolons, data)?,
            None if trial.traces.is_none() => PlayableTraces {
                skill_levels: BTreeMap::new(),
                node_unlocks: BTreeMap::new(),
            },
            None => return Err(ConversionError(format!("角色 {id} 的行迹缺少节点绑定"))),
        };
        let light_cone = expand_light_cone(owned, definition, trial, raw, data)?;
        let cone_data = data
            .light_cones
            .iter()
            .find(|entry| entry.id == light_cone.light_cone_id)
            .ok_or_else(|| {
                ConversionError(format!("缺少光锥 {} 的基础数据", light_cone.light_cone_id))
            })?;
        let mut values = AttributeValues::new();
        let mut sets = BTreeMap::<String, u8>::new();
        let relics = if let Some(character) = owned {
            let mapping = data
                .mappings
                .iter()
                .find(|entry| &entry.character_id == id)
                .ok_or_else(|| ConversionError(format!("缺少角色 {id} 的遗器映射模板")))?;
            let mut group =
                |slots: &[RelicSlot], template: &crate::reference::RelicGroupTemplate| {
                    let mut originals = Vec::with_capacity(slots.len());
                    let mut mapped = false;
                    for slot in slots {
                        let Some(instance_id) = character.relic_instance_ids.get(slot) else {
                            mapped = true;
                            continue;
                        };
                        let relic = raw
                            .relics
                            .iter()
                            .find(|entry| &entry.instance_id == instance_id)
                            .ok_or_else(|| ConversionError(format!("遗器 {instance_id} 不存在")))?;
                        mapped |= relic.level < super::RELIC_MAPPING_LEVEL;
                        originals.push(relic);
                    }
                    if mapped {
                        panel::add_values(&mut values, &template.stats);
                        *sets.entry(template.set_id.clone()).or_default() += slots.len() as u8;
                        Ok(PlayableRelicGroup::Mapped {
                            configuration: template.clone(),
                        })
                    } else {
                        let mut instance_ids = Vec::with_capacity(slots.len());
                        for relic in originals {
                            add_owned_relic(relic, data, &mut values)?;
                            *sets.entry(relic.set_id.clone()).or_default() += 1;
                            instance_ids.push(relic.instance_id.clone());
                        }
                        Ok(PlayableRelicGroup::Owned { instance_ids })
                    }
                };
            PlayableRelics::Partitioned {
                outer: group(
                    &[
                        RelicSlot::Head,
                        RelicSlot::Hands,
                        RelicSlot::Body,
                        RelicSlot::Feet,
                    ],
                    &mapping.outer,
                )?,
                planar: group(
                    &[RelicSlot::PlanarSphere, RelicSlot::LinkRope],
                    &mapping.planar,
                )?,
            }
        } else {
            panel::add_values(&mut values, &trial.relics.stats);
            *sets.entry(trial.relics.outer_set_id.clone()).or_default() += 4;
            *sets.entry(trial.relics.planar_set_id.clone()).or_default() += 2;
            PlayableRelics::TrialAggregate {
                configuration: trial.relics.clone(),
            }
        };
        for (set_id, pieces) in sets {
            let properties = data
                .set_properties
                .get(&set_id)
                .ok_or_else(|| ConversionError(format!("缺少套装 {set_id} 的常驻属性资料")))?;
            for (required, stats) in properties {
                if pieces >= *required {
                    panel::add_values(&mut values, stats);
                }
            }
        }
        if definition.path == Some(cone_data.path) {
            for (attribute, levels) in &cone_data.properties {
                *values.entry(*attribute).or_default() +=
                    levels[usize::from(light_cone.superimposition - 1)];
            }
        }
        if let Some(binding) = binding {
            let minors = data
                .minor_traces
                .iter()
                .find(|entry| &entry.character_id == id)
                .ok_or_else(|| ConversionError(format!("缺少角色 {id} 的小行迹属性")))?;
            for (suffix, bonus) in &minors.nodes {
                let label = format!("stat_{}", suffix - 200);
                let node = binding.nodes.get(&label).ok_or_else(|| {
                    ConversionError(format!("缺少角色 {id} 的小行迹 {suffix} 绑定"))
                })?;
                if traces.node_unlocks.get(node) == Some(&true) {
                    *values.entry(bonus.attribute).or_default() += bonus.value;
                }
            }
        }
        characters.push(PlayableCharacter {
            character_id: id.clone(),
            origin: match owned {
                Some(character) => CharacterOrigin::Owned {
                    raw_character_id: character.character_id.clone(),
                },
                None => CharacterOrigin::Trial,
            },
            eidolons,
            ability_version: binding.map(|entry| entry.ability_version),
            traces,
            light_cone,
            relics,
            panel: panel::calculate(definition, cone_data, &values)?,
        });
    }
    Ok(PlayableAccount {
        schema_version: PLAYABLE_ACCOUNT_SCHEMA_VERSION,
        config_id: request.config_id.clone(),
        raw_config_id: raw.config_id.clone(),
        game_version: data.manifest.game_version.clone(),
        season: data.manifest.season.clone(),
        data_snapshot: data.manifest.snapshot_id.clone(),
        rules_version: CONVERSION_RULES_VERSION.to_owned(),
        characters,
        owned_light_cones: raw.light_cones.iter().map(owned_light_cone).collect(),
        owned_relics: raw.relics.clone(),
    })
}

/// 把模板中的技能类别对应到绑定使用的来源标签。
fn skill_kind(label: &str) -> Option<SkillKind> {
    match label {
        "basic" => Some(SkillKind::Basic),
        "skill" => Some(SkillKind::Skill),
        "ult" => Some(SkillKind::Ultimate),
        "talent" => Some(SkillKind::Talent),
        "memosprite.skill" => Some(SkillKind::MemospriteSkill),
        "memosprite.talent" => Some(SkillKind::MemospriteTalent),
        "elation" => Some(SkillKind::Elation),
        "aha_biography" => Some(SkillKind::AhaBiography),
        _ => None,
    }
}

/// 绑定试用节点、转移本体进度并计算逐技能星魂加级。
fn expand_traces(
    owned: Option<&CharacterConfig>,
    trial: &TrialTemplate,
    binding: &TraceBinding,
    eidolons: u8,
    data: &PlayableData,
) -> Result<PlayableTraces, ConversionError> {
    let template = trial
        .traces
        .as_ref()
        .ok_or_else(|| ConversionError(format!("缺少角色 {} 的试用行迹", trial.character_id)))?;
    let mut floor = TraceConfig {
        skill_levels: BTreeMap::new(),
        node_unlocks: BTreeMap::new(),
    };
    for (label, node) in &binding.skills {
        let level = if node.max_level == 1 {
            1
        } else {
            skill_kind(label)
                .and_then(|kind| template.base_levels.get(&kind))
                .copied()
                .ok_or_else(|| {
                    ConversionError(format!(
                        "角色 {} 的试用配置缺少 {label}",
                        trial.character_id
                    ))
                })?
        };
        if level > node.max_level || level == 0 {
            return Err(ConversionError(format!(
                "角色 {} 的试用 {label} 等级超出范围",
                trial.character_id
            )));
        }
        floor.skill_levels.insert(node.node_id.clone(), level);
    }
    for (label, node) in &binding.nodes {
        let active = if let Some(index) = label.strip_prefix("stat_") {
            let suffix = index
                .parse::<u16>()
                .map_err(|_| ConversionError(format!("无效的小行迹标签 {label}")))?
                + 200;
            !template.inactive_minor_nodes.contains(&suffix)
        } else {
            true
        };
        floor.node_unlocks.insert(node.clone(), active);
    }
    let effective = if let Some(character) = owned {
        let version = character.ability_version.ok_or_else(|| {
            ConversionError(format!("原始角色 {} 缺少能力版本", character.character_id))
        })?;
        let source = data
            .traces
            .binding(&character.character_id, version)
            .ok_or_else(|| {
                ConversionError(format!(
                    "缺少原始角色 {} 的能力版本 {version}",
                    character.character_id
                ))
            })?;
        let mut rebased = TraceConfig {
            skill_levels: BTreeMap::new(),
            node_unlocks: BTreeMap::new(),
        };
        for (label, node) in &binding.skills {
            let level = if node.max_level == 1 {
                1
            } else if let Some(original) = source.skills.get(label) {
                let level = character
                    .traces
                    .skill_levels
                    .get(&original.node_id)
                    .copied()
                    .ok_or_else(|| {
                        ConversionError(format!(
                            "原始角色 {} 缺少技能节点 {} 的基础等级",
                            character.character_id, original.node_id
                        ))
                    })?;
                if level == 0 || level > original.max_level || level > node.max_level {
                    return Err(ConversionError(format!(
                        "原始角色 {} 的 {label} 基础等级超出范围",
                        character.character_id
                    )));
                }
                level
            } else {
                // 强化版本新增的技能在原版本中没有可转移的进度，采用已绑定的试用下限。
                floor.skill_levels[&node.node_id]
            };
            rebased.skill_levels.insert(node.node_id.clone(), level);
        }
        for (label, node) in &binding.nodes {
            if let Some(original) = source.nodes.get(label)
                && let Some(active) = character.traces.node_unlocks.get(original)
            {
                rebased.node_unlocks.insert(node.clone(), *active);
            }
        }
        raise_traces_to_trial(&rebased, &floor)
            .map_err(|error| ConversionError(error.to_string()))?
    } else {
        floor
    };
    for node in binding.nodes.values() {
        if !effective.node_unlocks.contains_key(node) {
            return Err(ConversionError(format!(
                "角色 {} 缺少节点 {} 的解锁状态",
                trial.character_id, node
            )));
        }
    }
    let mut skill_levels = BTreeMap::new();
    for node in binding.skills.values() {
        if node.skill_ids.is_empty() {
            return Err(ConversionError(format!(
                "行迹节点 {} 缺少技能 ID",
                node.node_id
            )));
        }
        for skill_id in &node.skill_ids {
            let extra: u8 = binding
                .rank_bonuses
                .range(..=eidolons)
                .filter_map(|(_, bonuses)| bonuses.get(skill_id))
                .sum();
            let level = effective.skill_levels[&node.node_id]
                .checked_add(extra)
                .ok_or_else(|| ConversionError(format!("技能 {skill_id} 的生效等级溢出")))?;
            if skill_levels.insert(skill_id.clone(), level).is_some() {
                return Err(ConversionError(format!(
                    "技能 {skill_id} 重复绑定到多个行迹节点"
                )));
            }
        }
    }
    Ok(PlayableTraces {
        skill_levels,
        node_unlocks: effective.node_unlocks,
    })
}

/// 保留本体光锥及叠影；空装使用命途默认光锥，试用直接使用试用光锥。
fn expand_light_cone(
    owned: Option<&CharacterConfig>,
    definition: &CharacterData,
    trial: &TrialTemplate,
    raw: &RawAccount,
    data: &PlayableData,
) -> Result<PlayableLightCone, ConversionError> {
    let result = match owned {
        Some(character) => match &character.light_cone_instance_id {
            Some(instance_id) => {
                let cone = raw
                    .light_cones
                    .iter()
                    .find(|entry| &entry.instance_id == instance_id)
                    .ok_or_else(|| ConversionError(format!("光锥实例 {instance_id} 不存在")))?;
                owned_light_cone(cone)
            }
            None => {
                let cone_id = definition
                    .path
                    .and_then(|path| data.default_light_cones.get(&path))
                    .ok_or_else(|| {
                        ConversionError(format!("缺少角色 {} 的默认映射光锥", definition.id))
                    })?;
                PlayableLightCone {
                    light_cone_id: cone_id.clone(),
                    superimposition: TEMPLATE_LIGHT_CONE_SUPERIMPOSITION,
                    origin: EquipmentOrigin::Mapped,
                }
            }
        },
        None => PlayableLightCone {
            light_cone_id: trial.light_cone_id.clone(),
            superimposition: TEMPLATE_LIGHT_CONE_SUPERIMPOSITION,
            origin: EquipmentOrigin::Trial,
        },
    };
    Ok(result)
}

/// 将本体光锥投影为游玩配置，原始等级由统一规则替代。
fn owned_light_cone(cone: &LightConeInstance) -> PlayableLightCone {
    PlayableLightCone {
        light_cone_id: cone.light_cone_id.clone(),
        superimposition: cone.superimposition,
        origin: EquipmentOrigin::Owned {
            instance_id: cone.instance_id.clone(),
        },
    }
}

/// 把保留的五星满级遗器主副词条计入属性合计。
fn add_owned_relic(
    relic: &RelicInstance,
    data: &PlayableData,
    values: &mut AttributeValues,
) -> Result<(), ConversionError> {
    let main = data
        .mainstats
        .get(&relic.mainstat)
        .ok_or_else(|| ConversionError(format!("缺少遗器 {} 的主词条数值", relic.instance_id)))?;
    *values.entry(relic.mainstat.into()).or_default() += main;
    for (attribute, value) in &relic.substats {
        *values.entry((*attribute).into()).or_default() += value;
    }
    Ok(())
}
