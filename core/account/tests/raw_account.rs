//! 统一格式的库存、单位、缺省语义和序列化约定。

use account_core::{
    InvalidRelicSlot, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, RelicMainStat, RelicSlot,
    RelicSubStat,
};
use serde_json::{Value, json};

/// 人工构造的模型样例，覆盖空装、部分配装、闲置库存和不同强化等级。
const EXAMPLE: &str = include_str!("fixtures/raw_account.json");

#[test]
fn preserves_inventory_partial_equipment_and_original_progression() {
    let account: RawAccount = serde_json::from_str(EXAMPLE).unwrap();

    assert_eq!(account.schema_version, RAW_ACCOUNT_SCHEMA_VERSION);
    assert_eq!(account.characters.len(), 3);
    assert_eq!(account.light_cones.len(), 2);
    assert_eq!(account.relics.len(), 2);
    let character = &account.characters[0];
    assert_eq!(
        (character.level, character.promotion, character.eidolons),
        (70, 5, 2)
    );
    assert_eq!(character.relic_instance_ids.len(), 1);
    assert_eq!(
        character.relic_instance_ids[&RelicSlot::Body],
        "example:relic:body"
    );
    assert!(account.characters[1].light_cone_instance_id.is_none());
    assert!(account.characters[1].relic_instance_ids.is_empty());
    assert_eq!(account.characters[1].character_id, "8002");
    assert_eq!(account.characters[2].character_id, "8004");
    assert_eq!(
        account.light_cones[1].light_cone_id,
        account.light_cones[0].light_cone_id
    );
    assert_ne!(
        account.light_cones[1].instance_id,
        account.light_cones[0].instance_id
    );
    assert_eq!((account.relics[0].rarity, account.relics[0].level), (5, 14));
    assert_eq!((account.relics[1].rarity, account.relics[1].level), (4, 0));
    assert_eq!(
        account.relics[0].substats[&RelicSubStat::HpPercent],
        0.034560003
    );
    assert_eq!(account.relics[0].substats[&RelicSubStat::Spd], 2.3);

    let encoded = serde_json::to_value(&account).unwrap();
    let original: Value = serde_json::from_str(EXAMPLE).unwrap();
    assert_eq!(encoded, original);
    assert_eq!(
        serde_json::from_value::<RawAccount>(encoded).unwrap(),
        account
    );
}

#[test]
fn distinguishes_unknown_trace_data_from_explicit_locked_nodes() {
    let account: RawAccount = serde_json::from_str(EXAMPLE).unwrap();
    assert_eq!(account.game_version, None);
    assert_eq!(account.data_snapshot, None);
    assert_eq!(account.characters[0].ability_version, Some(0));
    assert_eq!(account.characters[1].ability_version, None);
    assert_eq!(account.characters[2].ability_version, Some(1));
    let traces = &account.characters[0].traces;
    assert_eq!(traces.skill_levels.get("1004002"), Some(&8));
    assert_eq!(traces.skill_levels.get("1004003"), None);
    assert_eq!(traces.node_unlocks.get("1004007"), Some(&true));
    assert_eq!(traces.node_unlocks.get("1004008"), Some(&false));
    assert_eq!(traces.node_unlocks.get("1004009"), None);
}

#[test]
fn preserves_all_six_numeric_slot_codes() {
    let slots = [
        RelicSlot::Head,
        RelicSlot::Hands,
        RelicSlot::Body,
        RelicSlot::Feet,
        RelicSlot::PlanarSphere,
        RelicSlot::LinkRope,
    ];
    for (number, slot) in (1_u8..=6).zip(slots) {
        assert_eq!(RelicSlot::try_from(number), Ok(slot));
        assert_eq!(serde_json::to_value(slot).unwrap(), json!(number));
        assert_eq!(
            serde_json::from_value::<RelicSlot>(json!(number)).unwrap(),
            slot
        );
    }
    assert_eq!(RelicSlot::try_from(0), Err(InvalidRelicSlot(0)));
    assert_eq!(RelicSlot::try_from(7), Err(InvalidRelicSlot(7)));
    let error = serde_json::from_value::<RelicSlot>(json!(7)).unwrap_err();
    assert!(error.to_string().contains("遗器部位必须为 1～6，收到 7"));
}

#[test]
fn keeps_python_stat_keys_and_rejects_unmapped_source_keys() {
    let main_keys = [
        "HP",
        "ATK",
        "HP_",
        "ATK_",
        "DEF_",
        "SPD",
        "CRIT Rate_",
        "CRIT DMG_",
        "Effect Hit Rate_",
        "Outgoing Healing Boost_",
        "Break Effect_",
        "Energy Regeneration Rate_",
        "Physical DMG Boost_",
        "Fire DMG Boost_",
        "Ice DMG Boost_",
        "Lightning DMG Boost_",
        "Wind DMG Boost_",
        "Quantum DMG Boost_",
        "Imaginary DMG Boost_",
    ];
    for key in main_keys {
        let stat: RelicMainStat = serde_json::from_value(json!(key)).unwrap();
        assert_eq!(serde_json::to_value(stat).unwrap(), json!(key));
    }
    let sub_keys = [
        "HP",
        "ATK",
        "DEF",
        "HP_",
        "ATK_",
        "DEF_",
        "SPD",
        "CRIT Rate_",
        "CRIT DMG_",
        "Effect Hit Rate_",
        "Effect RES_",
        "Break Effect_",
    ];
    for key in sub_keys {
        let stat: RelicSubStat = serde_json::from_value(json!(key)).unwrap();
        assert_eq!(serde_json::to_value(stat).unwrap(), json!(key));
    }
    let error = serde_json::from_value::<RelicMainStat>(json!("CRIT Rate")).unwrap_err();
    assert!(error.to_string().contains("unknown variant `CRIT Rate`"));
    let error =
        serde_json::from_value::<RelicSubStat>(json!("Energy Regeneration Rate_")).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown variant `Energy Regeneration Rate_`")
    );
}

#[test]
fn reports_missing_required_fields_and_unadapted_export_fields() {
    let mut value: Value = serde_json::from_str(EXAMPLE).unwrap();
    value.as_object_mut().unwrap().remove("relics");
    let error = serde_json::from_value::<RawAccount>(value).unwrap_err();
    assert!(error.to_string().contains("missing field `relics`"));

    let mut value: Value = serde_json::from_str(EXAMPLE).unwrap();
    value["characters"][0]["ascension"] = json!(5);
    let error = serde_json::from_value::<RawAccount>(value).unwrap_err();
    assert!(error.to_string().contains("unknown field `ascension`"));
}
