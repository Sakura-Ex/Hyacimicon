//! 统一原始账号的结构、等级和库存归属校验。

use super::{RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// 原始账号违反的字段或库存规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAccountError(
    /// 具体错误原因。
    pub String,
);

impl fmt::Display for RawAccountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RawAccountError {}

/// 返回指定突破阶段可用的最低和最高等级。
pub(crate) fn level_range(promotion: u8) -> Option<(u8, u8)> {
    (promotion <= 6).then(|| {
        let min = if promotion == 0 {
            1
        } else {
            10 + promotion * 10
        };
        (min, 20 + promotion * 10)
    })
}

/// 校验可保存的原始账号及完整库存引用，保留已知或未知的资料版本。
pub fn validate_raw_account(raw: &RawAccount) -> Result<(), RawAccountError> {
    if raw.schema_version != RAW_ACCOUNT_SCHEMA_VERSION {
        return Err(RawAccountError("原始账号格式版本不受支持".into()));
    }
    if raw.config_id.trim().is_empty() {
        return Err(RawAccountError("原始账号配置 ID 不能为空".into()));
    }
    let mut ids = BTreeSet::new();
    for character in &raw.characters {
        let id = &character.character_id;
        if id.trim().is_empty() || !ids.insert(id) {
            return Err(RawAccountError(format!("原始角色 ID 为空或重复：{id}")));
        }
        if character.eidolons > 6
            || !level_range(character.promotion)
                .is_some_and(|(min, max)| (min..=max).contains(&character.level))
        {
            return Err(RawAccountError(format!(
                "角色 {id} 的等级、突破或星魂超出范围"
            )));
        }
    }
    for cone in &raw.light_cones {
        if !level_range(cone.promotion).is_some_and(|(min, max)| (min..=max).contains(&cone.level))
        {
            return Err(RawAccountError(format!(
                "光锥 {} 的等级与突破不相容",
                cone.instance_id
            )));
        }
    }
    validate_equipment(raw)
}

/// 核对库存光锥 ID 及跨角色的装备归属，避免同一实例同时装备。
pub(crate) fn validate_equipment(raw: &RawAccount) -> Result<(), RawAccountError> {
    let mut cone_ids = BTreeSet::new();
    for cone in &raw.light_cones {
        if !cone_ids.insert(&cone.instance_id) {
            return Err(RawAccountError(format!(
                "光锥实例 ID {} 重复",
                cone.instance_id
            )));
        }
        if !(1..=5).contains(&cone.superimposition) {
            return Err(RawAccountError(format!(
                "光锥 {} 的叠影超出 1～5",
                cone.light_cone_id
            )));
        }
    }
    let mut relics = BTreeMap::new();
    for relic in &raw.relics {
        if relics.insert(relic.instance_id.as_str(), relic).is_some() {
            return Err(RawAccountError(format!(
                "遗器实例 ID {} 重复",
                relic.instance_id
            )));
        }
        let max_level = RelicInstance::max_level_for_rarity(relic.rarity)
            .ok_or_else(|| RawAccountError(format!("遗器 {} 的品质超出范围", relic.instance_id)))?;
        if relic.level > max_level {
            return Err(RawAccountError(format!(
                "遗器 {} 的品质或强化等级超出范围",
                relic.instance_id
            )));
        }
        if !relic.mainstat.supports_slot(relic.slot) {
            return Err(RawAccountError(format!(
                "遗器 {} 的主词条与部位不匹配",
                relic.instance_id
            )));
        }
        if relic.substats.len() > RelicInstance::MAX_SUBSTATS {
            return Err(RawAccountError(format!(
                "遗器 {} 的副词条数量超出范围",
                relic.instance_id
            )));
        }
        let main: crate::playable::attributes::Attribute = relic.mainstat.into();
        if relic
            .substats
            .keys()
            .any(|key| crate::playable::attributes::Attribute::from(*key) == main)
        {
            return Err(RawAccountError(format!(
                "遗器 {} 的主副词条重复",
                relic.instance_id
            )));
        }
        if relic
            .substats
            .values()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(RawAccountError(format!(
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
                return Err(RawAccountError(format!(
                    "角色 {} 引用的光锥 {id} 不存在",
                    character.character_id
                )));
            }
            if !assigned_cones.insert(id) {
                return Err(RawAccountError(format!("光锥实例 {id} 被多个角色装备")));
            }
        }
        for (slot, id) in &character.relic_instance_ids {
            let relic = relics.get(id.as_str()).ok_or_else(|| {
                RawAccountError(format!(
                    "角色 {} 引用的遗器 {id} 不存在",
                    character.character_id
                ))
            })?;
            if relic.slot != *slot {
                return Err(RawAccountError(format!(
                    "角色 {} 的遗器 {id} 部位不一致",
                    character.character_id
                )));
            }
            if !assigned_relics.insert(id) {
                return Err(RawAccountError(format!("遗器实例 {id} 被多个角色装备")));
            }
        }
    }
    Ok(())
}
