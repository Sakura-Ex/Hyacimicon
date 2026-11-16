//! 核对原始资源，并按已确认的角色 ID 对照选择本体。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CharacterConfig, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance,
    reference::{CharacterData, CharacterKind, PlayableData},
};

use super::conversion::ConversionError;

/// 检查版本、装备归属和角色对照，返回玩法范围内的本体索引。
pub(super) fn owned_characters<'a>(
    raw: &'a RawAccount,
    data: &PlayableData,
    aliases: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, &'a CharacterConfig>, ConversionError> {
    if raw.schema_version != RAW_ACCOUNT_SCHEMA_VERSION {
        return Err(ConversionError("原始账号格式版本不受支持".to_owned()));
    }
    if let Some(version) = &raw.game_version
        && version != &data.manifest.game_version
    {
        return Err(ConversionError(
            "原始账号与目标资料的游戏版本不一致".to_owned(),
        ));
    }
    let definitions: BTreeMap<_, _> = data
        .characters
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    for target in aliases.values() {
        if !definitions.contains_key(target.as_str()) {
            return Err(ConversionError(format!("别名目标 {target} 未包含在资料中")));
        }
    }
    let mut raw_ids = BTreeSet::new();
    let mut owned = BTreeMap::new();
    for character in &raw.characters {
        if !raw_ids.insert(&character.character_id) {
            return Err(ConversionError(format!(
                "原始角色 ID {} 重复",
                character.character_id
            )));
        }
        let target = aliases
            .get(&character.character_id)
            .unwrap_or(&character.character_id);
        let Some(definition) = definitions.get(target.as_str()) else {
            continue;
        };
        if definition.kind == CharacterKind::GameplayExclusive {
            return Err(ConversionError(format!(
                "玩法专属角色 {target} 应从模板取得"
            )));
        }
        if owned.insert(target.clone(), character).is_some() {
            return Err(ConversionError(format!(
                "多个持有条目对应角色 {target}，需要先确定形态关系"
            )));
        }
    }
    validate_equipment(raw)?;
    Ok(owned)
}

/// 按稀有度确定星魂保底。
pub(super) fn mapped_eidolons(character: &CharacterData) -> Result<u8, ConversionError> {
    match character.rarity {
        Some(4) => Ok(6),
        Some(5) => Ok(0),
        _ => Err(ConversionError(format!(
            "角色 {} 缺少适用于星魂映射的四／五星稀有度",
            character.id
        ))),
    }
}

/// 核对库存光锥 ID 及跨角色的装备归属，避免同一实例同时装备。
fn validate_equipment(raw: &RawAccount) -> Result<(), ConversionError> {
    let mut cone_ids = BTreeSet::new();
    for cone in &raw.light_cones {
        if !cone_ids.insert(&cone.instance_id) {
            return Err(ConversionError(format!(
                "光锥实例 ID {} 重复",
                cone.instance_id
            )));
        }
        if !(1..=5).contains(&cone.superimposition) {
            return Err(ConversionError(format!(
                "光锥 {} 的叠影超出 1～5",
                cone.light_cone_id
            )));
        }
    }
    let mut relics = BTreeMap::new();
    for relic in &raw.relics {
        if relics.insert(relic.instance_id.as_str(), relic).is_some() {
            return Err(ConversionError(format!(
                "遗器实例 ID {} 重复",
                relic.instance_id
            )));
        }
        let max_level = RelicInstance::max_level_for_rarity(relic.rarity)
            .ok_or_else(|| ConversionError(format!("遗器 {} 的品质超出范围", relic.instance_id)))?;
        if relic.level > max_level {
            return Err(ConversionError(format!(
                "遗器 {} 的品质或强化等级超出范围",
                relic.instance_id
            )));
        }
        if !relic.mainstat.supports_slot(relic.slot) {
            return Err(ConversionError(format!(
                "遗器 {} 的主词条与部位不匹配",
                relic.instance_id
            )));
        }
        if relic.substats.len() > RelicInstance::MAX_SUBSTATS {
            return Err(ConversionError(format!(
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
            return Err(ConversionError(format!(
                "遗器 {} 的主副词条重复",
                relic.instance_id
            )));
        }
        if relic
            .substats
            .values()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(ConversionError(format!(
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
                return Err(ConversionError(format!(
                    "角色 {} 引用的光锥 {id} 不存在",
                    character.character_id
                )));
            }
            if !assigned_cones.insert(id) {
                return Err(ConversionError(format!("光锥实例 {id} 被多个角色装备")));
            }
        }
        for (slot, id) in &character.relic_instance_ids {
            let relic = relics.get(id.as_str()).ok_or_else(|| {
                ConversionError(format!(
                    "角色 {} 引用的遗器 {id} 不存在",
                    character.character_id
                ))
            })?;
            if relic.slot != *slot {
                return Err(ConversionError(format!(
                    "角色 {} 的遗器 {id} 部位不一致",
                    character.character_id
                )));
            }
            if !assigned_relics.insert(id) {
                return Err(ConversionError(format!("遗器实例 {id} 被多个角色装备")));
            }
        }
    }
    Ok(())
}
