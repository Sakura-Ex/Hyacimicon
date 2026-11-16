//! 核对文档数据、模板表达与来源选择计划。

use std::collections::{BTreeMap, BTreeSet};

use account_core::{
    RawAccount,
    attributes::Attribute,
    planning::{MappingAction, PlanRequest, plan_playable_account},
    playable::{CharacterOrigin, ConfigurationMode, PlayableRelics},
    reference::{MappingPart, PlayableData, SkillKind, Verification},
};

/// 模型阶段已有的人工原始账号。
fn account() -> RawAccount {
    serde_json::from_str(include_str!("fixtures/raw_account.json")).unwrap()
}

/// 显式建立模式，空选择表示各项映射尚待判断。
fn request(mode: ConfigurationMode) -> PlanRequest {
    PlanRequest {
        mode,
        character_aliases: BTreeMap::new(),
        mapping_decisions: BTreeMap::new(),
    }
}

#[test]
fn loads_all_documented_records_and_preserves_cross_references() {
    let data = PlayableData::bundled().unwrap();
    assert_eq!(data.manifest.game_version, "4.7");
    assert_eq!(data.manifest.season, "S2");
    assert_eq!(data.characters.data.len(), 61);
    assert_eq!(data.light_cones.data.len(), 172);
    assert_eq!(data.trials.data.len(), 61);
    assert_eq!(data.mappings.data.len(), 60);
    assert_eq!(data.gameplay_forms.data.len(), 216);
    assert_eq!(data.rules.data.default_light_cones.len(), 9);
    assert_eq!(
        (
            data.relics.data.mainstats.len(),
            data.relics.data.substats.len(),
            data.relics.data.sets.len()
        ),
        (19, 12, 64)
    );
    let character_ids: BTreeSet<_> = data.characters.data.iter().map(|entry| &entry.id).collect();
    let trial_ids: BTreeSet<_> = data
        .trials
        .data
        .iter()
        .map(|entry| &entry.character_id)
        .collect();
    assert_eq!(character_ids, trial_ids);
    assert_eq!(character_ids.len(), 61);
    let cone_ids: BTreeSet<_> = data
        .light_cones
        .data
        .iter()
        .map(|entry| &entry.id)
        .collect();
    let set_ids: BTreeSet<_> = data
        .relics
        .data
        .sets
        .iter()
        .map(|entry| &entry.id)
        .collect();
    for trial in &data.trials.data {
        assert!(cone_ids.contains(&trial.light_cone.light_cone_id));
        assert!(set_ids.contains(&trial.relics.outer_set_id));
        assert!(set_ids.contains(&trial.relics.planar_set_id));
    }
    for mapping in &data.mappings.data {
        assert!(character_ids.contains(&mapping.character_id));
        assert!(set_ids.contains(&mapping.outer.set_id));
        assert!(set_ids.contains(&mapping.planar.set_id));
    }
    let gameplay_ids: BTreeSet<_> = data
        .gameplay_forms
        .data
        .iter()
        .map(|entry| &entry.gameplay_id)
        .collect();
    assert_eq!(gameplay_ids.len(), 63);
    for form in &data.gameplay_forms.data {
        assert!(character_ids.contains(&form.character_id));
    }
    assert_eq!(data.manifest.files["trial-templates.json"].count, 61);
    assert_eq!(data.manifest.files["mapping-templates.json"].count, 60);
    assert_eq!(data.manifest.files["gameplay-forms.json"].count, 216);
}

#[test]
fn keeps_trial_and_mapping_values_distinct_and_retains_verification() {
    let data = PlayableData::bundled().unwrap();
    let trial = data
        .trials
        .data
        .iter()
        .find(|entry| entry.character_id == "1408")
        .unwrap();
    let mapping = data
        .mappings
        .data
        .iter()
        .find(|entry| entry.character_id == "1408")
        .unwrap();
    assert_eq!(
        trial.traces.as_ref().unwrap().base_levels[&SkillKind::Basic],
        6
    );
    assert_eq!(mapping.traces.base_levels[&SkillKind::Basic], 5);
    let welt = data
        .mappings
        .data
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.outer.set_id, "117");
    assert_eq!(welt.planar.set_id, "309");
    assert_eq!(welt.outer.stats[&Attribute::Speed], 29.632);
    assert!((welt.planar.stats[&Attribute::ImaginaryDamageBoost] - 0.388803).abs() < 1e-12);
    assert_eq!(welt.outer.verification, Verification::InGameVerified);
    assert_eq!(welt.planar.verification, Verification::InGameVerified);
    let evernight = data
        .mappings
        .data
        .iter()
        .find(|entry| entry.character_id == "1413")
        .unwrap();
    assert_eq!(evernight.outer.verification, Verification::PendingInGame);
    assert_eq!(evernight.planar.verification, Verification::InGameVerified);
    for entry in &data.mappings.data {
        let definition = data
            .characters
            .data
            .iter()
            .find(|definition| definition.id == entry.character_id)
            .unwrap();
        assert_eq!(
            entry.eidolons,
            if definition.rarity == Some(4) { 6 } else { 0 }
        );
    }
}

#[test]
fn preserves_aggregate_trial_relics_unknown_levels_and_special_characters() {
    let data = PlayableData::bundled().unwrap();
    let welt = data
        .trials
        .data
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.character_level, None);
    assert_eq!(welt.character_promotion, None);
    assert_eq!(welt.light_cone.level, None);
    assert_eq!(welt.light_cone.superimposition, 1);
    assert!((welt.relics.stats[&Attribute::ImaginaryDamageBoost] - 0.3888).abs() < 1e-12);
    let relics = PlayableRelics::TrialAggregate {
        template_id: "1004".to_owned(),
        configuration: welt.relics.clone(),
    };
    let encoded = serde_json::to_value(relics).unwrap();
    assert_eq!(encoded["kind"], "trial_aggregate");
    assert!(encoded["configuration"]["stats"].is_object());
    assert!(encoded.get("instance_ids").is_none());
    let pam = data
        .characters
        .data
        .iter()
        .find(|entry| entry.id == "6042")
        .unwrap();
    assert_eq!(pam.base.hp, 917.28);
    assert_eq!(pam.base.crit_rate, 1.0);
    assert_eq!(pam.base.aggro, Some(125));
    assert_eq!(pam.rarity, None);
    assert!(
        !data
            .mappings
            .data
            .iter()
            .any(|entry| entry.character_id == "6042")
    );
    assert_eq!(
        data.gameplay_forms
            .data
            .iter()
            .filter(|entry| entry.character_id == "1511")
            .count(),
        9
    );
    let aha = data
        .trials
        .data
        .iter()
        .find(|entry| entry.character_id == "1511")
        .unwrap();
    assert_eq!(
        aha.traces.as_ref().unwrap().base_levels[&SkillKind::AhaBiography],
        1
    );
    assert_eq!(data.rules.data.pending_triggers.len(), 6);
}

#[test]
fn fills_missing_roles_with_trial_sources_and_preserves_original_account() {
    let raw = account();
    let before = serde_json::to_value(&raw).unwrap();
    let data = PlayableData::bundled().unwrap();
    let plan =
        plan_playable_account(&raw, &data, &request(ConfigurationMode::OwnedWithTrials)).unwrap();
    assert_eq!(plan.characters.len(), 61);
    let welt = plan
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(
        welt.origin,
        CharacterOrigin::Owned {
            raw_character_id: "1004".to_owned()
        }
    );
    assert_eq!(welt.mapping_decisions.len(), 6);
    assert!(
        welt.mapping_decisions
            .values()
            .all(|action| *action == MappingAction::Undetermined)
    );
    let clara = plan
        .characters
        .iter()
        .find(|entry| entry.character_id == "1107")
        .unwrap();
    assert_eq!(
        clara.origin,
        CharacterOrigin::Trial {
            template_id: "1107".to_owned()
        }
    );
    assert!(clara.mapping_decisions.is_empty());
    let pam = plan
        .characters
        .iter()
        .find(|entry| entry.character_id == "6042")
        .unwrap();
    assert!(matches!(
        pam.origin,
        CharacterOrigin::GameplayExclusive { .. }
    ));
    assert_eq!(plan.unused_raw_character_ids, ["8002", "8004"]);
    assert_eq!(serde_json::to_value(&raw).unwrap(), before);
}

#[test]
fn supports_explicit_aliases_without_automatically_merging_forms() {
    let mut raw = account();
    raw.characters[1].character_id = "8010".to_owned();
    let data = PlayableData::bundled().unwrap();
    let mut options = request(ConfigurationMode::OwnedWithTrials);
    let without_alias = plan_playable_account(&raw, &data, &options).unwrap();
    assert!(
        without_alias
            .unused_raw_character_ids
            .contains(&"8010".to_owned())
    );
    options
        .character_aliases
        .insert("8010".to_owned(), "8009".to_owned());
    let with_alias = plan_playable_account(&raw, &data, &options).unwrap();
    let trailblazer = with_alias
        .characters
        .iter()
        .find(|entry| entry.character_id == "8009")
        .unwrap();
    assert_eq!(
        trailblazer.origin,
        CharacterOrigin::Owned {
            raw_character_id: "8010".to_owned()
        }
    );
    raw.characters[2].character_id = "8009".to_owned();
    let error = plan_playable_account(&raw, &data, &options).unwrap_err();
    assert!(error.0.contains("多个持有条目对应角色 8009"));
}

#[test]
fn keeps_inner_and_outer_mapping_choices_independent() {
    let raw = account();
    let data = PlayableData::bundled().unwrap();
    let mut options = request(ConfigurationMode::OwnedWithTrials);
    options.mapping_decisions.insert(
        "1004".to_owned(),
        BTreeMap::from([
            (MappingPart::OuterRelics, MappingAction::UseTemplate),
            (MappingPart::PlanarRelics, MappingAction::KeepOwned),
        ]),
    );
    let plan = plan_playable_account(&raw, &data, &options).unwrap();
    let welt = plan
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(
        welt.mapping_decisions[&MappingPart::OuterRelics],
        MappingAction::UseTemplate
    );
    assert_eq!(
        welt.mapping_decisions[&MappingPart::PlanarRelics],
        MappingAction::KeepOwned
    );
    assert_eq!(
        welt.mapping_decisions[&MappingPart::LightCone],
        MappingAction::Undetermined
    );
    let all_trial =
        plan_playable_account(&raw, &data, &request(ConfigurationMode::TrialsOnly)).unwrap();
    assert!(
        !all_trial
            .characters
            .iter()
            .any(|entry| matches!(entry.origin, CharacterOrigin::Owned { .. }))
    );
    assert!(
        all_trial
            .characters
            .iter()
            .all(|entry| entry.mapping_decisions.is_empty())
    );
}

#[test]
fn rejects_duplicate_ids_invalid_aliases_and_stale_choices() {
    let data = PlayableData::bundled().unwrap();
    let mut raw = account();
    raw.characters.push(raw.characters[0].clone());
    let error = plan_playable_account(&raw, &data, &request(ConfigurationMode::OwnedWithTrials))
        .unwrap_err();
    assert_eq!(error.0, "原始角色 ID 1004 重复");
    let mut options = request(ConfigurationMode::OwnedWithTrials);
    options
        .character_aliases
        .insert("8010".to_owned(), "9999".to_owned());
    assert_eq!(
        plan_playable_account(&account(), &data, &options)
            .unwrap_err()
            .0,
        "别名目标 9999 未包含在资料中"
    );
    let mut options = request(ConfigurationMode::OwnedWithTrials);
    options
        .mapping_decisions
        .insert("1107".to_owned(), BTreeMap::new());
    assert!(
        plan_playable_account(&account(), &data, &options)
            .unwrap_err()
            .0
            .contains("当前采用试用来源")
    );
    let mut raw = account();
    raw.game_version = Some("4.6".to_owned());
    assert_eq!(
        plan_playable_account(&raw, &data, &request(ConfigurationMode::OwnedWithTrials))
            .unwrap_err()
            .0,
        "原始账号与目标资料的游戏版本不一致"
    );
}
