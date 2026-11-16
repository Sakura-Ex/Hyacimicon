//! 核对文档数据、模板表达与来源选择计划。

use std::collections::{BTreeMap, BTreeSet};

use account_core::{
    RawAccount, RelicInstance, RelicMainStat, RelicSlot,
    playable::{
        CharacterOrigin, ConfigurationMode, PlayableRelics,
        attributes::Attribute,
        planning::{MappingAction, PlanRequest, mapped_eidolons, plan_playable_account},
    },
    reference::{MappingPart, PlayableData, SkillKind},
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
    assert_eq!(data.characters.len(), 61);
    assert_eq!(data.light_cones.len(), 172);
    assert_eq!(data.trials.len(), 61);
    assert_eq!(data.mappings.len(), 60);
    assert_eq!(data.gameplay_forms.len(), 63);
    assert_eq!(
        data.gameplay_forms
            .iter()
            .map(|form| form.stars.len())
            .sum::<usize>(),
        216
    );
    assert_eq!(data.default_light_cones.len(), 9);
    for (path, cone_id) in &data.default_light_cones {
        let cone = data
            .light_cones
            .iter()
            .find(|entry| entry.id == *cone_id)
            .unwrap();
        assert_eq!(cone.path, *path);
    }
    assert_eq!(data.minor_traces.len(), 60);
    for minor in &data.minor_traces {
        assert_eq!(
            minor.nodes.keys().copied().collect::<Vec<_>>(),
            (201..=210).collect::<Vec<_>>()
        );
    }
    let character_ids: BTreeSet<_> = data.characters.iter().map(|entry| &entry.id).collect();
    let trial_ids: BTreeSet<_> = data
        .trials
        .iter()
        .map(|entry| &entry.character_id)
        .collect();
    assert_eq!(character_ids, trial_ids);
    assert_eq!(character_ids.len(), 61);
    let cone_ids: BTreeSet<_> = data.light_cones.iter().map(|entry| &entry.id).collect();
    for trial in &data.trials {
        assert!(cone_ids.contains(&trial.light_cone_id));
        if let Some(traces) = &trial.traces {
            let minor = data
                .minor_traces
                .iter()
                .find(|entry| entry.character_id == trial.character_id)
                .unwrap();
            assert!(
                traces
                    .inactive_minor_nodes
                    .iter()
                    .all(|node| minor.nodes.contains_key(node))
            );
        }
    }
    for mapping in &data.mappings {
        assert!(character_ids.contains(&mapping.character_id));
    }
    let gameplay_ids: BTreeSet<_> = data
        .gameplay_forms
        .iter()
        .map(|entry| &entry.gameplay_id)
        .collect();
    assert_eq!(gameplay_ids.len(), 63);
    for form in &data.gameplay_forms {
        assert!(character_ids.contains(&form.character_id));
    }
}

#[test]
fn binds_the_last_two_minor_nodes_and_keeps_fully_unlocked_trials() {
    let data = PlayableData::bundled().unwrap();
    let welt = data
        .minor_traces
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.nodes[&201].attribute, Attribute::EffectHitRate);
    assert_eq!(welt.nodes[&201].value, 0.04);
    assert_eq!(welt.nodes[&209].attribute, Attribute::ImaginaryDamageBoost);
    assert_eq!(welt.nodes[&209].value, 0.064);
    assert_eq!(welt.nodes[&210].value, 0.08);
    let jingliu = data
        .minor_traces
        .iter()
        .find(|entry| entry.character_id == "1212")
        .unwrap();
    assert_eq!(jingliu.nodes[&209].attribute, Attribute::Speed);
    assert_eq!(jingliu.nodes[&209].value, 4.0);
    for trial in &data.trials {
        match trial.character_id.as_str() {
            "6042" => assert!(trial.traces.is_none()),
            "1408" | "1412" => assert!(
                trial
                    .traces
                    .as_ref()
                    .unwrap()
                    .inactive_minor_nodes
                    .is_empty()
            ),
            _ => assert_eq!(
                trial.traces.as_ref().unwrap().inactive_minor_nodes,
                [209, 210]
            ),
        }
    }
}

#[test]
fn keeps_trial_and_mapping_values_distinct() {
    let data = PlayableData::bundled().unwrap();
    let trial = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1408")
        .unwrap();
    assert_eq!(
        trial.traces.as_ref().unwrap().base_levels[&SkillKind::Basic],
        6
    );
    let welt = data
        .mappings
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.outer.set_id, "117");
    assert_eq!(welt.planar.set_id, "309");
    assert_eq!(welt.outer.stats[&Attribute::Speed], 29.632);
    assert!((welt.planar.stats[&Attribute::ImaginaryDamageBoost] - 0.388803).abs() < 1e-12);
}

#[test]
fn derives_mapped_eidolons_from_rarity_independently_of_trial_eidolons() {
    let data = PlayableData::bundled().unwrap();
    let natasha = data
        .characters
        .iter()
        .find(|entry| entry.id == "1105")
        .unwrap();
    assert_eq!(mapped_eidolons(natasha).unwrap(), 6);
    let trailblazer = data
        .characters
        .iter()
        .find(|entry| entry.id == "8009")
        .unwrap();
    assert_eq!(
        data.trials
            .iter()
            .find(|entry| entry.character_id == "8009")
            .unwrap()
            .eidolons,
        6
    );
    assert_eq!(mapped_eidolons(trailblazer).unwrap(), 0);
    let pam = data
        .characters
        .iter()
        .find(|entry| entry.id == "6042")
        .unwrap();
    assert_eq!(
        mapped_eidolons(pam).unwrap_err().0,
        "角色 6042 缺少适用于星魂映射的四／五星稀有度"
    );
}

#[test]
fn preserves_aggregate_trial_relics_and_special_characters() {
    let data = PlayableData::bundled().unwrap();
    let welt = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
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
        .iter()
        .find(|entry| entry.id == "6042")
        .unwrap();
    assert_eq!(pam.base.hp, 917.28);
    assert_eq!(pam.base.crit_rate(), 1.0);
    assert_eq!(pam.base.aggro, Some(125));
    assert_eq!(pam.rarity, None);
    assert!(
        !data
            .mappings
            .iter()
            .any(|entry| entry.character_id == "6042")
    );
    assert_eq!(
        data.gameplay_forms
            .iter()
            .filter(|entry| entry.character_id == "1511")
            .map(|form| form.stars.len())
            .sum::<usize>(),
        9
    );
    let aha = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1511")
        .unwrap();
    assert_eq!(
        aha.traces.as_ref().unwrap().base_levels[&SkillKind::AhaBiography],
        1
    );
    let normal = data
        .characters
        .iter()
        .find(|entry| entry.id == "1004")
        .unwrap();
    assert_eq!(normal.base.crit_rate(), 0.05);
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
    assert_eq!(welt.mapping_decisions.len(), 5);
    assert_eq!(
        welt.mapping_decisions[&MappingPart::OuterRelics],
        MappingAction::UseTemplate
    );
    assert!(
        welt.mapping_decisions
            .iter()
            .filter(|(part, _)| **part != MappingPart::OuterRelics)
            .all(|(_, action)| *action == MappingAction::Undetermined)
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

/// 为瓦尔特建立完整的六件 +15 配装，供逐部位检查触发条件。
fn fully_equipped_account() -> RawAccount {
    let mut raw = account();
    raw.relics.clear();
    raw.characters[0].relic_instance_ids.clear();
    for (slot, mainstat) in [
        (RelicSlot::Head, RelicMainStat::Hp),
        (RelicSlot::Hands, RelicMainStat::Atk),
        (RelicSlot::Body, RelicMainStat::CritRate),
        (RelicSlot::Feet, RelicMainStat::Spd),
        (RelicSlot::PlanarSphere, RelicMainStat::ImaginaryDmgBoost),
        (RelicSlot::LinkRope, RelicMainStat::EnergyRegenerationRate),
    ] {
        let instance_id = format!("example:full:{}", u8::from(slot));
        raw.characters[0]
            .relic_instance_ids
            .insert(slot, instance_id.clone());
        raw.relics.push(RelicInstance {
            instance_id,
            set_id: if u8::from(slot) <= 4 { "112" } else { "303" }.to_owned(),
            slot,
            rarity: 5,
            level: 15,
            mainstat,
            substats: BTreeMap::new(),
        });
    }
    raw
}

#[test]
fn maps_the_entire_group_when_any_one_piece_is_underleveled() {
    let data = PlayableData::bundled().unwrap();
    for changed_index in 0..6 {
        let mut raw = fully_equipped_account();
        raw.relics[changed_index].level = 12;
        let before = raw.clone();
        let plan = plan_playable_account(&raw, &data, &request(ConfigurationMode::OwnedWithTrials))
            .unwrap();
        let welt = plan
            .characters
            .iter()
            .find(|entry| entry.character_id == "1004")
            .unwrap();
        assert_eq!(
            welt.mapping_decisions[&MappingPart::OuterRelics],
            if changed_index < 4 {
                MappingAction::UseTemplate
            } else {
                MappingAction::KeepOwned
            }
        );
        assert_eq!(
            welt.mapping_decisions[&MappingPart::PlanarRelics],
            if changed_index >= 4 {
                MappingAction::UseTemplate
            } else {
                MappingAction::KeepOwned
            }
        );
        assert_eq!(raw, before);
    }
    let plan = plan_playable_account(
        &fully_equipped_account(),
        &data,
        &request(ConfigurationMode::OwnedWithTrials),
    )
    .unwrap();
    let welt = plan
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(
        welt.mapping_decisions[&MappingPart::OuterRelics],
        MappingAction::KeepOwned
    );
    assert_eq!(
        welt.mapping_decisions[&MappingPart::PlanarRelics],
        MappingAction::KeepOwned
    );
}

#[test]
fn rejects_invalid_relic_references_and_conflicting_manual_choices() {
    let data = PlayableData::bundled().unwrap();
    let options = request(ConfigurationMode::OwnedWithTrials);
    let mut raw = fully_equipped_account();
    raw.relics[0].level = 12;
    raw.relics[1].slot = RelicSlot::Feet;
    assert_eq!(
        plan_playable_account(&raw, &data, &options).unwrap_err().0,
        "角色 1004 的遗器 example:full:2 部位不一致"
    );
    raw.relics.remove(1);
    assert_eq!(
        plan_playable_account(&raw, &data, &options).unwrap_err().0,
        "角色 1004 引用的遗器 example:full:2 不存在"
    );
    let mut raw = fully_equipped_account();
    raw.relics[0].level = 12;
    let mut options = options;
    options.mapping_decisions.insert(
        "1004".to_owned(),
        BTreeMap::from([(MappingPart::OuterRelics, MappingAction::KeepOwned)]),
    );
    assert_eq!(
        plan_playable_account(&raw, &data, &options).unwrap_err().0,
        "角色 1004 的 OuterRelics 选择与遗器等级规则冲突"
    );
}
