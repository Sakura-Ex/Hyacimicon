//! 核对原始资源，并按已确认的角色 ID 对照选择本体。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CharacterConfig, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount,
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
    crate::raw::validation::validate_equipment(raw).map_err(|error| ConversionError(error.0))?;
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
