//! 装备实例、词条单位和来源装备归属的转换。

use std::collections::{BTreeMap, BTreeSet};

use super::{ImportError, bounded, invalid, numeric_id, progression, source};
use crate::{
    CharacterConfig, LightConeInstance, RelicInstance, RelicMainStat, RelicSlot, RelicSubStat,
};

/// 转换全部装备库存并在角色上建立唯一的装备引用。
pub(super) fn convert(
    export: &source::Export,
    config_id: &str,
    indices: &BTreeMap<String, usize>,
    characters: &mut [CharacterConfig],
) -> Result<(Vec<LightConeInstance>, Vec<RelicInstance>), ImportError> {
    let mut light_cones = Vec::with_capacity(export.light_cones.len());
    let mut relics = Vec::with_capacity(export.relics.len());
    let mut cone_ids = BTreeSet::new();
    for (index, cone) in export.light_cones.iter().enumerate() {
        let path = format!("light_cones[{index}]");
        numeric_id(&cone.id, &format!("{path}.id"))?;
        unique_id(&cone.uid, &path, &mut cone_ids)?;
        progression(cone.level, cone.ascension, &path)?;
        bounded(
            cone.superimposition,
            1,
            5,
            &format!("{path}.superimposition"),
        )?;
        let instance_id = format!("{config_id}:light-cone:{}", cone.uid);
        if let Some(character) = owner(&cone.location, &path, indices, characters)? {
            if character.light_cone_instance_id.is_some() {
                return Err(invalid(
                    format!("{path}.location"),
                    "该角色已装备另一件光锥",
                ));
            }
            character.light_cone_instance_id = Some(instance_id.clone());
        }
        light_cones.push(LightConeInstance {
            instance_id,
            light_cone_id: cone.id.clone(),
            level: cone.level,
            promotion: cone.ascension,
            superimposition: cone.superimposition,
        });
    }
    let mut relic_ids = BTreeSet::new();
    for (index, relic) in export.relics.iter().enumerate() {
        let path = format!("relics[{index}]");
        unique_id(&relic.uid, &path, &mut relic_ids)?;
        numeric_id(&relic.set_id, &format!("{path}.set_id"))?;
        let max_level = RelicInstance::max_level_for_rarity(relic.rarity)
            .ok_or_else(|| invalid(format!("{path}.rarity"), "遗器品质超出支持范围（2～5）"))?;
        bounded(relic.level, 0, max_level, &format!("{path}.level"))?;
        let slot = slot(&relic.slot, &format!("{path}.slot"))?;
        let (mainstat, main_key) = mainstat(slot, &relic.mainstat, &format!("{path}.mainstat"))?;
        let instance_id = format!("{config_id}:relic:{}", relic.uid);
        let substats = substats(relic, &main_key, &path)?;
        if let Some(character) = owner(&relic.location, &path, indices, characters)?
            && character
                .relic_instance_ids
                .insert(slot, instance_id.clone())
                .is_some()
        {
            return Err(invalid(
                format!("{path}.location"),
                "该角色的此部位已装备另一件遗器",
            ));
        }
        relics.push(RelicInstance {
            instance_id,
            set_id: relic.set_id.clone(),
            slot,
            rarity: relic.rarity,
            level: relic.level,
            mainstat,
            substats,
        });
    }
    Ok((light_cones, relics))
}

/// 检查同一类装备的来源实例标识唯一。
fn unique_id<'a>(id: &'a str, path: &str, seen: &mut BTreeSet<&'a str>) -> Result<(), ImportError> {
    let field = format!("{path}._uid");
    numeric_id(id, &field)?;
    if !seen.insert(id) {
        return Err(invalid(field, "装备实例 ID 重复"));
    }
    Ok(())
}

/// 按 location 查找角色；空字符串保留闲置状态。
fn owner<'a>(
    location: &str,
    path: &str,
    indices: &BTreeMap<String, usize>,
    characters: &'a mut [CharacterConfig],
) -> Result<Option<&'a mut CharacterConfig>, ImportError> {
    if location.is_empty() {
        return Ok(None);
    }
    let index = indices.get(location).ok_or_else(|| {
        invalid(
            format!("{path}.location"),
            "装备归属引用了导出中不存在的角色",
        )
    })?;
    // indices 在角色转换时按同一列表的顺序建立，索引始终位于切片范围内。
    Ok(Some(&mut characters[*index]))
}

/// 映射来源的英文部位标签。
fn slot(value: &str, path: &str) -> Result<RelicSlot, ImportError> {
    match value {
        "Head" => Ok(RelicSlot::Head),
        "Hands" => Ok(RelicSlot::Hands),
        "Body" => Ok(RelicSlot::Body),
        "Feet" => Ok(RelicSlot::Feet),
        "Planar Sphere" => Ok(RelicSlot::PlanarSphere),
        "Link Rope" => Ok(RelicSlot::LinkRope),
        _ => Err(invalid(path, "未知遗器部位标签")),
    }
}

/// 结合部位区分来源省略的百分比主词条后缀。
fn mainstat(
    slot: RelicSlot,
    value: &str,
    path: &str,
) -> Result<(RelicMainStat, String), ImportError> {
    use RelicMainStat as Stat;
    let key = match (slot, value) {
        (RelicSlot::Head, "HP") | (RelicSlot::Hands, "ATK") | (_, "SPD") => value.to_owned(),
        _ => format!("{value}_"),
    };
    let stat: Stat = serde_json::from_value(serde_json::Value::String(key.clone()))
        .map_err(|_| invalid(path, "未知主词条标签"))?;
    if !stat.supports_slot(slot) {
        return Err(invalid(path, "主词条与遗器部位不匹配"));
    }
    Ok((stat, key))
}

/// 转换当前副词条并检查换算后的数值。
fn substats(
    relic: &source::Relic,
    main_key: &str,
    path: &str,
) -> Result<BTreeMap<RelicSubStat, f64>, ImportError> {
    if relic.substats.len() > RelicInstance::MAX_SUBSTATS {
        return Err(invalid(format!("{path}.substats"), "当前副词条最多四条"));
    }
    let mut stats = BTreeMap::new();
    for (index, stat) in relic.substats.iter().enumerate() {
        let field = format!("{path}.substats[{index}]");
        let key: RelicSubStat = serde_json::from_value(serde_json::Value::String(stat.key.clone()))
            .map_err(|_| invalid(format!("{field}.key"), "未知副词条标签"))?;
        if stat.key == main_key {
            return Err(invalid(format!("{field}.key"), "副词条与主词条重复"));
        }
        let value = if stat.key.ends_with('_') {
            stat.value / 100.0
        } else {
            stat.value
        };
        if !value.is_finite() || value <= 0.0 {
            return Err(invalid(
                format!("{field}.value"),
                "副词条换算后的数值应为有限正数",
            ));
        }
        if stats.insert(key, value).is_some() {
            return Err(invalid(format!("{field}.key"), "当前副词条属性重复"));
        }
    }
    Ok(stats)
}
