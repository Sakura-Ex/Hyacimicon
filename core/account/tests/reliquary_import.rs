//! 来源适配的固定预期、库存关系和错误路径。

use account_core::{
    RAW_ACCOUNT_SCHEMA_VERSION, RelicMainStat, RelicSlot, RelicSubStat,
    import::reliquary::{ImportError, import_reliquary_archive},
};
use serde_json::{Value, json};

/// 人工构造的 v4 来源样例，覆盖增强角色、忆灵、欢愉技及闲置库存。
const EXPORT: &str = include_str!("fixtures/reliquary_v4.json");

/// 返回可通过来源 JSON 接口注入反例的独立样例值。
fn fixture() -> Value {
    serde_json::from_str(EXPORT).unwrap()
}

/// 核对具体字段和失败原因，避免把其他解析错误当成预期拒绝。
fn assert_field_error(value: Value, path: &str, reason: &str) {
    let error = import_reliquary_archive(&value.to_string(), "fixture").unwrap_err();
    match error {
        ImportError::Field {
            path: actual_path,
            message,
        } => {
            assert_eq!(actual_path, path);
            assert!(message.contains(reason), "实际原因：{message}");
        }
        other => panic!("预期字段错误，实际为 {other}"),
    }
}

#[test]
fn imports_complete_inventory_and_keeps_source_fields_separate() {
    let imported = import_reliquary_archive(EXPORT, "fixture").unwrap();
    let account = &imported.account;
    assert_eq!(account.schema_version, RAW_ACCOUNT_SCHEMA_VERSION);
    assert_eq!(account.config_id, "fixture");
    assert_eq!(
        (
            account.game_version.as_ref(),
            account.data_snapshot.as_ref()
        ),
        (None, None)
    );
    assert_eq!(
        (
            account.characters.len(),
            account.light_cones.len(),
            account.relics.len()
        ),
        (3, 2, 2)
    );
    let welt = &account.characters[0];
    assert_eq!((welt.level, welt.promotion, welt.eidolons), (60, 4, 1));
    assert_eq!(
        welt.light_cone_instance_id.as_deref(),
        Some("fixture:light-cone:1")
    );
    assert_eq!(welt.relic_instance_ids[&RelicSlot::Feet], "fixture:relic:1");
    assert!(account.characters[1].light_cone_instance_id.is_none());
    assert!(account.characters[1].relic_instance_ids.is_empty());
    assert_eq!(account.light_cones[1].light_cone_id, "23004");
    assert_eq!(account.light_cones[1].instance_id, "fixture:light-cone:2");
    assert_eq!(account.relics[0].mainstat, RelicMainStat::HpPercent);
    assert_eq!(account.relics[1].mainstat, RelicMainStat::Hp);
    assert_eq!((account.relics[1].rarity, account.relics[1].level), (4, 0));
    assert_eq!(account.relics[0].substats.len(), 3);
    assert!((account.relics[0].substats[&RelicSubStat::CritRate] - 0.029160001).abs() < 1e-12);
    assert!((account.relics[0].substats[&RelicSubStat::DefPercent] - 0.0486).abs() < 1e-12);
    assert_eq!(account.relics[0].substats[&RelicSubStat::Spd], 2.3);
    assert!(
        !account.relics[0]
            .substats
            .contains_key(&RelicSubStat::BreakEffect)
    );
    assert!(!account.relics[0].substats.contains_key(&RelicSubStat::Atk));
    assert_eq!(imported.source_data, fixture());
    assert_eq!(imported.report.format_version, 4);
    assert_eq!(imported.report.build, "0.19.0");
    assert_eq!(
        imported.report.trace_bindings_snapshot,
        "reliquary-traces-4.7-fe44559408d9"
    );
    assert_eq!(imported.report.default_techniques, 3);
    assert!(imported.report.warnings.is_empty());
}

#[test]
fn binds_ability_versions_special_skills_and_all_trace_labels() {
    let imported = import_reliquary_archive(EXPORT, "fixture").unwrap();
    let welt = &imported.account.characters[0];
    assert_eq!(welt.ability_version, Some(1));
    assert_eq!(welt.traces.skill_levels["11004001"], 3);
    assert_eq!(welt.traces.skill_levels["11004002"], 6);
    assert_eq!(welt.traces.skill_levels["11004007"], 1);
    assert!(!welt.traces.skill_levels.contains_key("1004001"));
    assert_eq!(welt.traces.node_unlocks.len(), 13);
    assert!(welt.traces.node_unlocks["11004201"]);
    assert!(!welt.traces.node_unlocks["11004210"]);
    assert!(!welt.traces.node_unlocks["11004102"]);
    let trailblazer = &imported.account.characters[1];
    assert_eq!(trailblazer.traces.skill_levels["8008301"], 6);
    assert_eq!(trailblazer.traces.skill_levels["8008302"], 6);
    assert!(trailblazer.traces.node_unlocks["8008501"]);
    assert_eq!(
        imported.account.characters[2].traces.skill_levels["1501420"],
        10
    );
}

#[test]
fn preserves_missing_trace_data_as_unknown_with_paths() {
    let mut value = fixture();
    value["characters"][0]["traces"]
        .as_object_mut()
        .unwrap()
        .remove("stat_1");
    value["characters"][0]["skills"]
        .as_object_mut()
        .unwrap()
        .remove("skill");
    let imported = import_reliquary_archive(&value.to_string(), "fixture").unwrap();
    let welt = &imported.account.characters[0];
    assert!(!welt.traces.node_unlocks.contains_key("11004201"));
    assert!(!welt.traces.skill_levels.contains_key("11004002"));
    let paths: Vec<_> = imported
        .report
        .warnings
        .iter()
        .map(|warning| warning.path.as_str())
        .collect();
    assert_eq!(
        paths,
        ["characters[0].skills.skill", "characters[0].traces.stat_1"]
    );
}

#[test]
fn rejects_unsupported_source_versions_and_unmapped_character_versions() {
    let mut value = fixture();
    value["source"] = json!("other_exporter");
    assert_field_error(value, "source", "reliquary_archiver");
    let mut value = fixture();
    value["version"] = json!(5);
    value.as_object_mut().unwrap().remove("characters");
    assert_field_error(value, "version", "版本 4");
    let mut value = fixture();
    value["characters"][0]["ability_version"] = json!(99);
    assert_field_error(value, "characters[0].ability_version", "未覆盖");
    let mut value = fixture();
    value["characters"][0]["id"] = json!("9999");
    assert_field_error(value, "characters[0].ability_version", "未覆盖角色 9999");
}

#[test]
fn rejects_duplicate_ids_and_equipment_assignment_conflicts() {
    let mut value = fixture();
    value["characters"][1]["id"] = json!("1004");
    assert_field_error(value, "characters[1].id", "重复");
    let mut value = fixture();
    value["light_cones"][1]["_uid"] = json!("1");
    assert_field_error(value, "light_cones[1]._uid", "重复");
    let mut value = fixture();
    value["relics"][1]["_uid"] = json!("1");
    assert_field_error(value, "relics[1]._uid", "重复");
    let mut value = fixture();
    value["light_cones"][1]["location"] = json!("1004");
    assert_field_error(value, "light_cones[1].location", "另一件光锥");
    let mut value = fixture();
    value["relics"][1]["location"] = json!("1004");
    value["relics"][1]["slot"] = json!("Feet");
    assert_field_error(value, "relics[1].location", "另一件遗器");
    let mut value = fixture();
    value["relics"][0]["location"] = json!("9999");
    assert_field_error(value, "relics[0].location", "不存在的角色");
}

#[test]
fn rejects_invalid_ids_ranges_and_source_labels() {
    let cases = [
        (
            "/light_cones/0/_uid",
            json!("01"),
            "light_cones[0]._uid",
            "正整数字符串",
        ),
        (
            "/characters/0/level",
            json!(80),
            "characters[0].level",
            "50～60",
        ),
        (
            "/characters/0/eidolon",
            json!(7),
            "characters[0].eidolon",
            "0～6",
        ),
        (
            "/light_cones/0/ascension",
            json!(7),
            "light_cones[0].ascension",
            "0～6",
        ),
        (
            "/light_cones/0/superimposition",
            json!(0),
            "light_cones[0].superimposition",
            "1～5",
        ),
        ("/relics/1/level", json!(15), "relics[1].level", "0～12"),
        (
            "/relics/0/slot",
            json!("Boots"),
            "relics[0].slot",
            "未知遗器部位",
        ),
        (
            "/relics/1/mainstat",
            json!("SPD"),
            "relics[1].mainstat",
            "部位不匹配",
        ),
        (
            "/relics/0/substats/0/key",
            json!("Unknown"),
            "relics[0].substats[0].key",
            "未知副词条",
        ),
        (
            "/relics/0/substats/0/value",
            json!(-1),
            "relics[0].substats[0].value",
            "有限正数",
        ),
        (
            "/characters/0/skills/skill",
            json!(11),
            "characters[0].skills.skill",
            "1～10",
        ),
        (
            "/characters/0/traces/special",
            json!(true),
            "characters[0].traces.special",
            "没有对应行迹",
        ),
    ];
    for (pointer, replacement, path, reason) in cases {
        let mut value = fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_field_error(value, path, reason);
    }
    let mut value = fixture();
    value["characters"][0]["skills"]["unknown"] = json!(1);
    assert_field_error(value, "characters[0].skills.unknown", "未知技能标签");
}

#[test]
fn rejects_duplicate_substats_and_main_substat_overlap() {
    let mut value = fixture();
    value["relics"][0]["substats"][1]["key"] = json!("SPD");
    assert_field_error(value, "relics[0].substats[1].key", "属性重复");
    let mut value = fixture();
    value["relics"][0]["substats"][1]["key"] = json!("HP_");
    assert_field_error(value, "relics[0].substats[1].key", "主词条重复");
}

#[test]
fn reports_malformed_json_and_missing_required_fields() {
    let error = import_reliquary_archive("{", "fixture").unwrap_err();
    assert!(matches!(error, ImportError::Json(_)));
    let mut value = fixture();
    value["characters"][0]
        .as_object_mut()
        .unwrap()
        .remove("level");
    let error = import_reliquary_archive(&value.to_string(), "fixture").unwrap_err();
    match error {
        ImportError::Json(error) => assert!(error.to_string().contains("missing field `level`")),
        other => panic!("预期字段缺失错误，实际为 {other}"),
    }
}

#[test]
fn preserves_unknown_extensions_and_reports_their_paths() {
    let mut value = fixture();
    value["characters"][0]["future_configuration"] = json!({"variant": 2});
    let imported = import_reliquary_archive(&value.to_string(), "fixture").unwrap();
    assert_eq!(imported.source_data, value);
    assert_eq!(imported.report.warnings.len(), 1);
    assert_eq!(
        imported.report.warnings[0].path,
        "characters[0].future_configuration"
    );
    assert!(imported.report.warnings[0].message.contains("待适配"));
}
