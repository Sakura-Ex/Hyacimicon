//! 通过真实开发入口核对来源、种子和解析结果。
use battle_core::diagnostics::ScenarioFile;
use serde_json::{json, Value};
use std::{path::PathBuf, process::Command};

/// 返回仓库内开发场景的绝对路径。
fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// 调用由 diagnostics 功能构建的实际命令行程序。
fn invoke(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hcwa-battle"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn development_cli_preserves_full_width_seeds_and_reports() {
    let file = example("duel.json");
    let parsed = invoke(&["inspect", &file]);
    assert!(parsed.status.success());
    let source: Value = serde_json::from_slice(&parsed.stdout).unwrap();
    let a = invoke(&["simulate", &file, "18446744073709551615"]);
    let b = invoke(&["simulate", &file, "18446744073709551615"]);
    assert!(a.status.success());
    assert_eq!(a.stdout, b.stdout);
    let result: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(result["seed"], "18446744073709551615");
    assert_eq!(result["scenario_sha256"], source["scenario_sha256"]);
}

#[test]
fn development_cli_reuses_batch_statistics() {
    let output = invoke(&["batch", &example("duel.json"), "64", "7"]);
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["completed"], 64);
    assert_eq!(result["ledger"].as_array().unwrap().len(), 64);
    assert_eq!(
        invoke(&["batch", &example("duel.json"), "0", "7"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn diagnostic_metadata_rejects_explicitly_unsupported_input() {
    let source: Value = serde_json::from_str(include_str!("../examples/duel.json")).unwrap();
    for (field, value) in [
        ("schema_version", json!(2)),
        ("ruleset", json!("unknown")),
        ("unsupported_mechanics", json!(["unmodeled_effect"])),
        ("unsupported_mechanics", json!(null)),
    ] {
        let mut invalid = source.clone();
        invalid[field] = value;
        let bytes = serde_json::to_vec(&invalid).unwrap();
        let error = ScenarioFile::from_slice(&bytes).unwrap_err();
        assert!(
            error.contains(field),
            "expected the invalid field {field} in {error}"
        );
    }

    let mut without_metadata = source;
    for field in ["schema_version", "ruleset", "unsupported_mechanics"] {
        without_metadata.as_object_mut().unwrap().remove(field);
    }
    ScenarioFile::from_slice(&serde_json::to_vec(&without_metadata).unwrap()).unwrap();
}

#[test]
fn diagnostic_settlement_requires_complete_positive_weights_and_valid_parameters() {
    let mut source: Value = serde_json::from_str(include_str!("../examples/duel.json")).unwrap();
    let enemy = source["waves"][0][0]["id"].as_str().unwrap().to_owned();
    source["mechanics"]["settlement"] = json!({
        "name": "weighted_kills_flat_penalty_v1",
        "enemy_weights": { (enemy.clone()): 1.0 },
        "minimum_progress": 0.5,
        "hp_before": 60.0,
        "hp_cap": 100.0,
        "failure_loss": 20.0,
        "recovery": 3.0,
    });
    ScenarioFile::from_slice(&serde_json::to_vec(&source).unwrap()).unwrap();

    for (field, value) in [
        ("name", json!("unknown")),
        ("enemy_weights", json!({})),
        ("enemy_weights", json!({ "wrong_enemy": 1.0 })),
        ("enemy_weights", json!({ (enemy): 0.0 })),
        ("minimum_progress", json!(1.1)),
        ("hp_before", json!(101.0)),
        ("recovery", json!(-1.0)),
    ] {
        let mut invalid = source.clone();
        invalid["mechanics"]["settlement"][field] = value;
        let error = ScenarioFile::from_slice(&serde_json::to_vec(&invalid).unwrap()).unwrap_err();
        assert!(
            error.contains(field),
            "expected the invalid settlement field {field} in {error}"
        );
    }

    let duplicate = source["waves"][0][0].clone();
    source["waves"][0].as_array_mut().unwrap().push(duplicate);
    let error = ScenarioFile::from_slice(&serde_json::to_vec(&source).unwrap()).unwrap_err();
    assert!(error.contains("duplicate enemy ID"), "{error}");
}

#[test]
fn diagnostic_parse_rejects_conflicting_keys_before_map_insertion() {
    let source = include_str!("../examples/anaxa-s2-full-battle.json");
    for (key, previous, path) in [
        ("schema_version", r#""schema_version": 2"#, "$"),
        ("ruleset", r#""rule\u0073et": "other""#, "$"),
        (
            "unsupported_mechanics",
            r#""unsupported_mechanics": ["unknown"]"#,
            "$",
        ),
        ("mechanics", r#""mechanics": {}"#, "$"),
        ("settlement", r#""settlement": {}"#, "$.mechanics"),
        (
            "failure_loss",
            r#""failure_loss": -50"#,
            "$.mechanics.settlement",
        ),
        (
            "enemy_1",
            r#""enemy_1": 0"#,
            "$.mechanics.settlement.enemy_weights",
        ),
    ] {
        let marker = format!("{key:?}:");
        let start = if key == "enemy_1" {
            source.find(r#""enemy_weights":"#).unwrap()
        } else {
            0
        };
        let (prefix, object) = source.split_at(start);
        let edited = object.replacen(&marker, &format!("{previous}, {marker}"), 1);
        let invalid = format!("{prefix}{edited}");
        assert_ne!(invalid, source, "missing fixture key {key}");
        let error = ScenarioFile::from_slice(invalid.as_bytes())
            .err()
            .unwrap_or_else(|| panic!("accepted duplicate key {key} at {path}"));
        assert!(
            error.contains(&format!("duplicate field {key:?} at {path}")),
            "{error}"
        );
    }
}
