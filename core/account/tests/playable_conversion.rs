//! 完整转换的数值、保底和资源约束回归。

use std::collections::BTreeMap;

use account_core::{
    CharacterConfig, LightConeInstance, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance,
    RelicMainStat, RelicSlot, RelicSubStat, TraceConfig,
    playable::{
        CharacterOrigin, EquipmentOrigin, PlayableAccount, PlayableRelicGroup, PlayableRelics,
        conversion::{ConversionRequest, convert_playable_account},
    },
    reference::PlayableData,
};

/// 瓦尔特普通能力版本的最小本体账号，保留低等级 S5 光锥。
fn account() -> RawAccount {
    let mut node_unlocks = BTreeMap::new();
    for suffix in [
        101, 102, 103, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210,
    ] {
        node_unlocks.insert(format!("1004{suffix}"), suffix == 210);
    }
    RawAccount {
        schema_version: RAW_ACCOUNT_SCHEMA_VERSION,
        config_id: "conversion-test".to_owned(),
        game_version: None,
        data_snapshot: None,
        characters: vec![CharacterConfig {
            character_id: "1004".to_owned(),
            level: 1,
            promotion: 0,
            eidolons: 2,
            ability_version: Some(0),
            traces: TraceConfig {
                skill_levels: BTreeMap::from([
                    ("1004001".to_owned(), 6),
                    ("1004002".to_owned(), 1),
                    ("1004003".to_owned(), 9),
                    ("1004004".to_owned(), 10),
                    ("1004007".to_owned(), 1),
                ]),
                node_unlocks,
            },
            light_cone_instance_id: Some("test:cone".to_owned()),
            relic_instance_ids: BTreeMap::new(),
        }],
        light_cones: vec![LightConeInstance {
            instance_id: "test:cone".to_owned(),
            light_cone_id: "21001".to_owned(),
            level: 1,
            promotion: 0,
            superimposition: 5,
        }],
        relics: vec![],
    }
}

/// 完整转换请求。
fn request() -> ConversionRequest {
    ConversionRequest {
        config_id: "playable-test".to_owned(),
        character_aliases: BTreeMap::new(),
    }
}

/// 核对按配置精度计算的数值。
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn saved_playable_accounts_round_trip_without_conversion_version_labels() {
    let account =
        convert_playable_account(&account(), &PlayableData::bundled().unwrap(), &request())
            .unwrap();
    let current = serde_json::to_value(account).unwrap();
    assert!(current.get("rules_version").is_none());
    assert!(current.get("schema_version").is_none());

    let mut old = current.clone();
    old["schema_version"] = 3.into();
    old["rules_version"] = "s2-4.7-v2".into();
    for saved in [old, current.clone()] {
        let parsed: PlayableAccount = serde_json::from_value(saved).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), current);
    }
    let mut mistyped = current;
    mistyped["owned_light_cone"] = serde_json::json!([]);
    let error = serde_json::from_value::<PlayableAccount>(mistyped).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown field `owned_light_cone`")
    );
}

#[test]
fn converts_owned_progression_to_a_complete_panel_without_reducing_resources() {
    let raw = account();
    let original = raw.clone();
    let data = PlayableData::bundled().unwrap();
    let result = convert_playable_account(&raw, &data, &request()).unwrap();
    let welt = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.eidolons, 2);
    assert_eq!(welt.ability_version, Some(1));
    assert_eq!(welt.traces.skill_levels["1100401"], 6);
    assert_eq!(welt.traces.skill_levels["1100402"], 8);
    assert_eq!(welt.traces.skill_levels["1100403"], 9);
    assert_eq!(welt.traces.skill_levels["1100404"], 10);
    assert!(welt.traces.node_unlocks["11004101"]);
    assert!(welt.traces.node_unlocks["11004210"]);
    assert!(!welt.traces.node_unlocks["11004209"]);
    let cone = &welt.light_cone;
    assert_eq!(cone.light_cone_id, "21001");
    assert_eq!(cone.superimposition, 5);
    assert!(matches!(cone.origin, EquipmentOrigin::Owned { .. }));
    close(welt.panel.hp.base, 2077.992);
    close(welt.panel.hp.added, 1109.5616448);
    close(welt.panel.attack.base, 1097.208);
    close(welt.panel.attack.added, 1040.0910912);
    close(welt.panel.defense.added, 204.145515);
    close(welt.panel.speed.base, 102.0);
    close(welt.panel.speed.added, 36.532);
    close(welt.panel.crit_rate, 0.6398);
    close(welt.panel.effect_hit_rate, 0.4744);
    assert_eq!(result.characters.len(), 61);
    assert_eq!(result.owned_light_cones.len(), 1);
    assert_eq!(result.owned_light_cones[0].light_cone_id, "21001");
    assert_eq!(result.owned_light_cones[0].superimposition, 5);
    assert_eq!(
        result.owned_light_cones[0].origin,
        EquipmentOrigin::Owned {
            instance_id: "test:cone".to_owned()
        }
    );
    assert_eq!(raw, original);
    let encoded = serde_json::to_value(&welt.panel.hp).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 2);
}

/// 为测试账号增加四件满级外圈，内圈保留空装。
fn equip_outer(raw: &mut RawAccount) {
    for (slot, mainstat, substat, value) in [
        (
            RelicSlot::Head,
            RelicMainStat::Hp,
            RelicSubStat::AtkPercent,
            0.10,
        ),
        (
            RelicSlot::Hands,
            RelicMainStat::Atk,
            RelicSubStat::HpPercent,
            0.10,
        ),
        (
            RelicSlot::Body,
            RelicMainStat::CritRate,
            RelicSubStat::Spd,
            2.6,
        ),
        (
            RelicSlot::Feet,
            RelicMainStat::Spd,
            RelicSubStat::EffectHitRate,
            0.0324,
        ),
    ] {
        let id = format!("test:relic:{}", u8::from(slot));
        raw.characters[0]
            .relic_instance_ids
            .insert(slot, id.clone());
        raw.relics.push(RelicInstance {
            instance_id: id,
            set_id: "102".to_owned(),
            slot,
            rarity: 5,
            level: 15,
            mainstat,
            substats: BTreeMap::from([(substat, value)]),
        });
    }
}

#[test]
fn counts_owned_mainstats_substats_and_set_bonuses_once_with_mapped_inner_relics() {
    let mut raw = account();
    equip_outer(&mut raw);
    let result =
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &request()).unwrap();
    let welt = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert!(matches!(&welt.relics, PlayableRelics::Partitioned {
        outer: PlayableRelicGroup::Owned { instance_ids }, planar: PlayableRelicGroup::Mapped { .. }
    } if instance_ids.len() == 4));
    close(welt.panel.hp.added, 913.3992);
    close(welt.panel.attack.added, 1068.179616);
    close(welt.panel.speed.added, 40.652);
    close(welt.panel.crit_rate, 0.54148);
}

#[test]
fn light_cone_base_speed_scales_with_percent_speed_only_when_its_path_matches() {
    let mut raw = account();
    let role = &mut raw.characters[0];
    role.character_id = "1408".to_owned();
    role.traces.skill_levels = [(1, 6), (2, 10), (3, 10), (4, 10), (7, 1)]
        .into_iter()
        .map(|(suffix, level)| (format!("1408{suffix:03}"), level))
        .collect();
    role.traces.node_unlocks = [
        101, 102, 103, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210,
    ]
    .into_iter()
    .map(|suffix| (format!("1408{suffix}"), true))
    .collect();
    equip_outer(&mut raw);
    for relic in &mut raw.relics {
        relic.substats.clear();
        if matches!(relic.slot, RelicSlot::Body | RelicSlot::Feet) {
            relic.mainstat = RelicMainStat::HpPercent;
        }
    }
    let data = PlayableData::bundled().unwrap();
    for (cone_id, rank, base, added) in [
        ("23044", 1, 106.0, 18.26),
        ("23044", 5, 114.0, 18.74),
        ("23036", 1, 94.0, 17.54),
    ] {
        raw.light_cones[0].light_cone_id = cone_id.to_owned();
        raw.light_cones[0].superimposition = rank;
        let result = convert_playable_account(&raw, &data, &request()).unwrap();
        let phainon = result
            .characters
            .iter()
            .find(|role| role.character_id == "1408")
            .unwrap();
        close(phainon.panel.speed.base, base);
        close(phainon.panel.speed.added, added);
        close(phainon.panel.speed.total(), base + added);
    }
}

#[test]
fn empty_owned_roster_is_filled_with_trial_eidolons_skills_and_equipment() {
    let mut raw = account();
    raw.characters.clear();
    let before = raw.clone();
    let result =
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &request()).unwrap();
    assert_eq!(raw, before);
    assert_eq!(result.characters.len(), 61);
    assert_eq!(result.owned_light_cones[0].superimposition, 5);
    assert!(
        result
            .characters
            .iter()
            .all(|entry| !matches!(entry.origin, CharacterOrigin::Owned { .. }))
    );
    let welt = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.eidolons, 0);
    assert_eq!(welt.light_cone.superimposition, 1);
    let phainon = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1408")
        .unwrap();
    assert_eq!(phainon.traces.skill_levels["140801"], 6);
    assert_eq!(phainon.traces.skill_levels["140802"], 10);
    let aha = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1511")
        .unwrap();
    assert_eq!(aha.traces.skill_levels["151131"], 1);
    let pam = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "6042")
        .unwrap();
    assert!(pam.traces.skill_levels.is_empty());
}

#[test]
fn four_star_eidolons_are_raised_to_six_before_per_skill_rank_bonuses() {
    let mut raw = account();
    let character = &mut raw.characters[0];
    character.character_id = "1105".to_owned();
    character.eidolons = 0;
    character.light_cone_instance_id = None;
    character.traces.skill_levels = BTreeMap::from([
        ("1105001".to_owned(), 1),
        ("1105002".to_owned(), 9),
        ("1105003".to_owned(), 10),
        ("1105004".to_owned(), 1),
        ("1105007".to_owned(), 1),
    ]);
    character.traces.node_unlocks = [
        101, 102, 103, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210,
    ]
    .into_iter()
    .map(|suffix| (format!("1105{suffix}"), false))
    .collect();
    let result =
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &request()).unwrap();
    let natasha = result
        .characters
        .iter()
        .find(|entry| entry.character_id == "1105")
        .unwrap();
    assert_eq!(natasha.eidolons, 6);
    assert_eq!(natasha.traces.skill_levels["110501"], 6);
    assert_eq!(natasha.traces.skill_levels["110502"], 11);
    assert_eq!(natasha.traces.skill_levels["110503"], 12);
    assert_eq!(natasha.traces.skill_levels["110504"], 10);
    assert_eq!(natasha.light_cone.light_cone_id, "21021");
    assert!(matches!(natasha.light_cone.origin, EquipmentOrigin::Mapped));
}

#[test]
fn reports_missing_required_data_and_shared_equipment_instead_of_filling_zeroes() {
    let mut raw = account();
    equip_outer(&mut raw);
    let mut data = PlayableData::bundled().unwrap();
    data.mainstats.clear();
    assert!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0
            .contains("主词条数值")
    );
    let mut raw = account();
    let mut second = raw.characters[0].clone();
    second.character_id = "1105".to_owned();
    raw.characters.push(second);
    assert_eq!(
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &request())
            .unwrap_err()
            .0,
        "光锥实例 test:cone 被多个角色装备"
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
fn fills_trial_sources_and_keeps_owned_inventory_available() {
    let raw = account();
    let before = raw.clone();
    let result =
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &request()).unwrap();
    assert_eq!(
        result
            .characters
            .iter()
            .filter(|role| matches!(role.origin, CharacterOrigin::Owned { .. }))
            .count(),
        1
    );
    assert_eq!(
        result
            .characters
            .iter()
            .filter(|role| role.origin == CharacterOrigin::Trial)
            .count(),
        60
    );
    assert_eq!(
        result
            .characters
            .iter()
            .find(|role| role.character_id == "6042")
            .unwrap()
            .origin,
        CharacterOrigin::Trial
    );
    assert_eq!(raw, before);
}

#[test]
fn explicit_form_aliases_preserve_source_and_do_not_raise_five_stars_to_trial_eidolons() {
    let mut raw = account();
    let role = &mut raw.characters[0];
    role.character_id = "8010".to_owned();
    role.eidolons = 0;
    role.light_cone_instance_id = None;
    role.traces.skill_levels = [(1, 6), (2, 10), (3, 10), (4, 10), (420, 10)]
        .into_iter()
        .map(|(suffix, level)| (format!("8010{suffix:03}"), level))
        .collect();
    role.traces.node_unlocks = [
        101, 102, 103, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210,
    ]
    .into_iter()
    .map(|suffix| (format!("8010{suffix}"), false))
    .collect();
    let data = PlayableData::bundled().unwrap();
    let mut options = request();
    let without = convert_playable_account(&raw, &data, &options).unwrap();
    let trial = without
        .characters
        .iter()
        .find(|role| role.character_id == "8009")
        .unwrap();
    assert_eq!(trial.origin, CharacterOrigin::Trial);
    assert_eq!(trial.eidolons, 6);
    options
        .character_aliases
        .insert("8010".to_owned(), "8009".to_owned());
    let with = convert_playable_account(&raw, &data, &options).unwrap();
    let owned = with
        .characters
        .iter()
        .find(|role| role.character_id == "8009")
        .unwrap();
    assert_eq!(
        owned.origin,
        CharacterOrigin::Owned {
            raw_character_id: "8010".to_owned()
        }
    );
    assert_eq!(owned.eidolons, 0);
    assert_eq!(owned.traces.skill_levels["800901"], 6);
    assert_eq!(owned.traces.skill_levels["800920"], 10);
    let mut duplicate = raw.characters[0].clone();
    duplicate.character_id = "8009".to_owned();
    raw.characters.push(duplicate);
    assert!(
        convert_playable_account(&raw, &data, &options)
            .unwrap_err()
            .0
            .contains("多个持有条目对应角色 8009")
    );
}

#[test]
fn maps_each_whole_group_for_every_underleveled_or_empty_slot() {
    let data = PlayableData::bundled().unwrap();
    let options = request();
    let full = convert_playable_account(&fully_equipped_account(), &data, &options).unwrap();
    let welt = full
        .characters
        .iter()
        .find(|role| role.character_id == "1004")
        .unwrap();
    assert!(matches!(
        &welt.relics,
        PlayableRelics::Partitioned {
            outer: PlayableRelicGroup::Owned { .. },
            planar: PlayableRelicGroup::Owned { .. }
        }
    ));
    for changed in 0..6 {
        for empty in [false, true] {
            let mut raw = fully_equipped_account();
            if empty {
                raw.characters[0]
                    .relic_instance_ids
                    .remove(&raw.relics[changed].slot);
            } else {
                raw.relics[changed].level = 12;
            }
            let before = raw.clone();
            let output = convert_playable_account(&raw, &data, &options).unwrap();
            let welt = output
                .characters
                .iter()
                .find(|role| role.character_id == "1004")
                .unwrap();
            let PlayableRelics::Partitioned { outer, planar } = &welt.relics else {
                panic!("本体应分别保存内外圈")
            };
            assert_eq!(
                matches!(outer, PlayableRelicGroup::Mapped { .. }),
                changed < 4
            );
            assert_eq!(
                matches!(planar, PlayableRelicGroup::Mapped { .. }),
                changed >= 4
            );
            assert_eq!(raw, before);
        }
    }
}

#[test]
fn rejects_duplicate_ids_unknown_alias_targets_and_incompatible_versions() {
    let data = PlayableData::bundled().unwrap();
    let mut raw = account();
    raw.characters.push(raw.characters[0].clone());
    assert_eq!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0,
        "原始角色 ID 1004 重复"
    );
    let mut options = request();
    options
        .character_aliases
        .insert("8010".to_owned(), "9999".to_owned());
    assert_eq!(
        convert_playable_account(&account(), &data, &options)
            .unwrap_err()
            .0,
        "别名目标 9999 未包含在资料中"
    );
    let mut raw = account();
    raw.game_version = Some("4.6".to_owned());
    assert_eq!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0,
        "原始账号与目标资料的游戏版本不一致"
    );
}

#[test]
fn rejects_bad_relic_references_even_when_another_piece_triggers_mapping() {
    let data = PlayableData::bundled().unwrap();
    let mut raw = fully_equipped_account();
    raw.relics.push(raw.relics[0].clone());
    assert_eq!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0,
        "遗器实例 ID example:full:1 重复"
    );
    let mut raw = fully_equipped_account();
    raw.relics[0].level = 12;
    raw.relics[1].slot = RelicSlot::Feet;
    raw.relics[1].mainstat = RelicMainStat::Spd;
    assert_eq!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0,
        "角色 1004 的遗器 example:full:2 部位不一致"
    );
    raw.relics.remove(1);
    assert_eq!(
        convert_playable_account(&raw, &data, &request())
            .unwrap_err()
            .0,
        "角色 1004 引用的遗器 example:full:2 不存在"
    );
}

#[test]
fn rejects_panels_that_overflow_when_finite_percent_bonuses_are_scaled() {
    let data = PlayableData::bundled().unwrap();
    for stat in [
        RelicSubStat::HpPercent,
        RelicSubStat::AtkPercent,
        RelicSubStat::DefPercent,
    ] {
        let mut raw = fully_equipped_account();
        raw.relics[0].substats.insert(stat, 1e308);
        let Err(error) = convert_playable_account(&raw, &data, &request()) else {
            panic!("百分比属性乘基础值溢出时应返回错误");
        };
        assert_eq!(error.0, "角色 1004 的静态面板包含非有限数值");
    }
    let mut raw = fully_equipped_account();
    raw.relics[0]
        .substats
        .insert(RelicSubStat::AtkPercent, 1e100);
    let result = convert_playable_account(&raw, &data, &request()).unwrap();
    let welt = result
        .characters
        .iter()
        .find(|role| role.character_id == "1004")
        .unwrap();
    assert!(welt.panel.attack.total().is_finite());
}

#[test]
fn rejects_panels_whose_finite_base_and_added_overflow_when_combined() {
    let mut data = PlayableData::bundled().unwrap();
    data.characters
        .iter_mut()
        .find(|role| role.id == "1004")
        .unwrap()
        .base
        .attack = 1e308;
    let mut raw = fully_equipped_account();
    raw.relics[0].substats.insert(RelicSubStat::AtkPercent, 1.0);
    let Err(error) = convert_playable_account(&raw, &data, &request()) else {
        panic!("基础值和加成相加溢出时应返回错误");
    };
    assert_eq!(error.0, "角色 1004 的静态面板包含非有限数值");
}
