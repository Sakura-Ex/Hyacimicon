//! 通过真实开发入口核对来源、种子和解析结果。
use battle_core::diagnostics::{
    replay::{replay, ReplayScript},
    ScenarioFile,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};

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

/// 逐字段核对同一操作路径的最终结果，并验证步骤区间覆盖提交后的所有事件。
fn checked_replay(name: &str, expected_steps: usize) -> Value {
    let scenario = example(&format!("{name}.json"));
    let operations = example(&format!("{name}-operations.json"));
    let output = invoke(&["replay", &scenario, "42", &operations]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let again = invoke(&["replay", &scenario, "42", &operations]);
    assert!(again.status.success());
    assert_eq!(output.stdout, again.stdout);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let events = report["events"].as_array().unwrap();
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps.len(), expected_steps);
    assert_eq!(report["trace_truncated"], false);

    let mut cursor = steps[0]["event_start"].as_u64().unwrap() as usize;
    for (id, step) in steps.iter().enumerate() {
        let after_submit = step["event_after_submit"].as_u64().unwrap() as usize;
        let end = step["event_end"].as_u64().unwrap() as usize;
        assert_eq!(step["decision_id"], id);
        assert_eq!(step["event_start"], cursor);
        assert!(cursor <= after_submit && after_submit <= end && end <= events.len());
        let choice = &step["choice"];
        if choice["kind"] == "action" {
            assert!(step["legal"].as_array().unwrap().iter().any(|candidate| {
                candidate["actor"] == choice["actor"] && candidate["action"] == choice["action"]
            }));
            let execution = if step["point"] == "insert" {
                assert_eq!(events[cursor]["kind"], "request_queued");
                &events[after_submit..end]
            } else {
                &events[cursor..after_submit]
            };
            assert!(execution.iter().any(|event| {
                event["kind"] == "action"
                    && event["actor"] == choice["actor"]
                    && event["action"] == choice["action"]
            }));
        } else {
            assert_eq!(step["point"], "insert");
            assert_eq!(after_submit, cursor);
        }
        cursor = end;
    }
    assert_eq!(cursor, events.len());
    let baseline = invoke(&["simulate", &scenario, "42"]);
    assert!(baseline.status.success());
    let mut battle = report.clone();
    battle.as_object_mut().unwrap().remove("steps");
    assert_eq!(
        battle,
        serde_json::from_slice::<Value>(&baseline.stdout).unwrap()
    );
    report
}

#[test]
fn scripted_replay_checks_wave_transition_and_ultimate_payment() {
    let report = checked_replay("team-waves", 49);
    assert_eq!(report["outcome"], "victory");
    assert_eq!(report["waves_cleared"], 2);
    assert_eq!(report["actions"], 18);
    assert!((report["remaining_av"].as_f64().unwrap() - 954.5454545454545).abs() < 1e-9);
    assert_eq!(report["steps"][22]["wave"], 1);
    let insert = &report["steps"][23];
    assert_eq!(insert["wave"], 2);
    assert_eq!(
        insert["observation"]["now"],
        report["steps"][22]["observation"]["now"]
    );
    let events = &report["events"];
    assert_eq!(insert["event_start"], 65);
    assert_eq!(insert["event_after_submit"], 66);
    assert_eq!(events[65]["amount"], 70.0);
    assert_eq!(events[66]["kind"], "energy_cost");
    assert_eq!(events[66]["amount"], 60.0);
    assert_eq!(events[67]["amount"], 10.0);
    assert_eq!(events[69]["target"], "wave2_boss");
    assert_eq!(events[69]["amount"], 480.0);
    assert_eq!(events[72]["amount"], 240.0);
    assert_eq!(events[145]["amount"], 480.0);
    assert_eq!(events[147]["amount"], 420.0);
}

#[test]
fn scripted_replay_checks_revival_penalties_and_budget_exhaustion() {
    let report = checked_replay("revival-budget", 13);
    assert_eq!(report["outcome"], "defeat");
    assert_eq!(report["termination"], "av_budget_exhausted");
    assert_eq!(report["actions"], 4);
    assert_eq!(report["elapsed_av"], 300.0);
    assert_eq!(report["penalty_av"], 50.0);
    assert_eq!(report["remaining_av"], 0.0);
    assert_eq!(report["units"][0]["hp"], 100.0);
    assert_eq!(report["units"][0]["revivals_left"], 0);
    let events = &report["events"];
    for (step, death) in [(4, 14), (10, 33)] {
        assert_eq!(report["steps"][step]["event_after_submit"], death + 4);
        assert_eq!(events[death]["kind"], "death");
        assert_eq!(events[death + 1]["kind"], "revive");
        assert_eq!(events[death + 1]["amount"], 100.0);
        assert_eq!(events[death + 2]["kind"], "revival_av_penalty");
        assert_eq!(events[death + 2]["amount"], 25.0);
        assert_eq!(events[death]["av"], events[death + 2]["av"]);
    }
}

#[test]
fn scripted_replay_reports_the_first_disagreement() {
    let file =
        ScenarioFile::from_slice(&fs::read(example("revival-budget.json")).unwrap()).unwrap();
    let script: Value =
        serde_json::from_slice(&fs::read(example("revival-budget-operations.json")).unwrap())
            .unwrap();
    let run = |script| replay(&file, 42, &serde_json::from_value(script).unwrap()).unwrap_err();
    let mut changed = script.clone();
    changed["steps"].as_array_mut().unwrap().truncate(1);
    assert!(run(changed).contains("script ended before decision 1 (NormalTurn)"));

    let mut changed = script.clone();
    changed["steps"]
        .as_array_mut()
        .unwrap()
        .push(script["steps"][0].clone());
    assert!(run(changed).contains("battle finished before script step 13"));

    let mut changed = script.clone();
    changed["steps"][1]["choice"] = json!({"kind":"action", "actor":"striker", "action":"basic"});
    assert!(run(changed).contains("script step 1, decision 1: illegal operation"));

    let mut changed = script.clone();
    changed["steps"][4]["expect"][0]["target"] = json!("wrong-target");
    assert!(run(changed).contains("script step 4, decision 4: event 15 mismatch"));

    let mut changed = script.clone();
    changed["steps"][4]["expect"][1]["amount"] = json!(26);
    assert_eq!(
        run(changed),
        "script step 4, decision 4: event 16 (revival_av_penalty) amount mismatch: expected 26, got 25"
    );

    let mut changed = script;
    changed["steps"][4]["expect"][1]["amount"] = Value::Null;
    let error = serde_json::from_value::<ReplayScript>(changed).unwrap_err();
    assert!(error
        .to_string()
        .contains("invalid type: null, expected f64"));
}

#[test]
fn scripted_replay_rejects_action_fields_on_skip() {
    let operations =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/replay-invalid-skip.json");
    let output = invoke(&[
        "replay",
        &example("revival-budget.json"),
        "42",
        operations.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["status"], "error");
    assert!(error["error"]
        .as_str()
        .unwrap()
        .starts_with("operations schema: unknown field `actor`"));
}

#[test]
fn scripted_replay_uses_a_legal_choice_other_than_the_first() {
    let mut file =
        ScenarioFile::from_slice(&fs::read(example("team-waves.json")).unwrap()).unwrap();
    file.battle.max_actions = 2;
    let mut script: Value =
        serde_json::from_slice(&fs::read(example("team-waves-operations.json")).unwrap()).unwrap();
    script["steps"].as_array_mut().unwrap().truncate(5);
    script["steps"][4]["choice"]["action"] = json!("basic");
    let script = serde_json::from_value(script).unwrap();
    let report = serde_json::to_value(replay(&file, 42, &script).unwrap()).unwrap();

    assert_eq!(report["steps"][4]["legal"][0]["action"], "skill");
    let hit = &report["events"][11];
    assert_eq!(hit["kind"], "damage");
    assert_eq!(hit["action"], "basic");
    assert_eq!(hit["target"], "wave1_a");
    assert_eq!(hit["critical"], true);
    assert_eq!(hit["amount"], 240.0);
    assert_eq!(report["outcome"], "incomplete");
    assert_eq!(report["termination"], "action_limit");
}
