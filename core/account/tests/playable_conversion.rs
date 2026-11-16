//! 完整转换的数值、保底和资源约束回归。

use std::collections::BTreeMap;

use account_core::{
    CharacterConfig, LightConeInstance, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicInstance,
    RelicMainStat, RelicSlot, RelicSubStat, TraceConfig,
    playable::{
        CharacterOrigin, ConfigurationMode, EquipmentOrigin, PlayableRelicGroup, PlayableRelics,
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

/// 普通资源模式的完整转换请求。
fn request() -> ConversionRequest {
    ConversionRequest {
        config_id: "playable-test".to_owned(),
        mode: ConfigurationMode::OwnedWithTrials,
        character_aliases: BTreeMap::new(),
    }
}

/// 核对按配置精度计算的数值。
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
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
fn trial_mode_uses_trial_eidolons_skills_and_equipment_for_all_characters() {
    let raw = account();
    let mut options = request();
    options.mode = ConfigurationMode::TrialsOnly;
    let result =
        convert_playable_account(&raw, &PlayableData::bundled().unwrap(), &options).unwrap();
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
    assert!(matches!(
        natasha.light_cone.origin,
        EquipmentOrigin::Mapped { .. }
    ));
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
