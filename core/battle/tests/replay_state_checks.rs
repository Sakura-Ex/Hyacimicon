#![cfg(feature = "diagnostics")]
//! Reconcile independent state observations at the real replay boundaries.

mod common;

use battle_core::{
    diagnostics::{
        replay::{replay, ReplayScript},
        ScenarioFile,
    },
    model::*,
};
use common::{action, damage, scenario, unit};
use serde_json::{json, Value};

fn run(file: &ScenarioFile, checks: Value) -> Result<Value, String> {
    let script: ReplayScript = serde_json::from_value(checks).map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(replay(file, 42, &script)?).unwrap())
}

fn file(input: &Scenario) -> ScenarioFile {
    ScenarioFile::from_slice(&serde_json::to_vec(input).unwrap()).unwrap()
}

fn skip() -> Value {
    json!({"point":"insert","choice":{"kind":"skip"}})
}

#[test]
fn same_av_boundaries_and_last_advance_use_their_actual_states() {
    let mut hero = unit("hero", 100.0);
    hero.initial_energy = 20.0;
    hero.actions = vec![
        Action {
            energy_gain: 20.0,
            sp_gain: 1,
            ..action("strike", vec![damage(100.0, Target::AbilityTarget)])
        },
        Action {
            kind: ActionKind::Ultimate,
            energy_cost: 20.0,
            ..action("ultimate", vec![damage(60.0, Target::AbilityTarget)])
        },
    ];
    let mut input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    input.max_av = 120.0;
    input.ally_skill_points.initial = 3;
    let file = file(&input);
    let script = json!({
        "schema_version":1,
        "expect_opening":{"wave":1,"cycle":1,"elapsed_av":0,"remaining_av":120,"ally_skill_points":3,
                          "units":[{"id":"hero","energy":20},{"id":"enemy","hp":1000}]},
        "steps":[
            {"point":"insert", "choice":{"kind":"action","actor":"hero","action":"ultimate","target":"enemy"},
             "expect_after":{"elapsed_av":0,"units":[{"id":"hero","energy":0},{"id":"enemy","hp":940}]}},
            skip(),
            {"point":"normal_turn","choice":{"kind":"action","actor":"hero","action":"strike"},
             "expect_after":{"elapsed_av":100,"remaining_av":20,"ally_skill_points":4,
                             "units":[{"id":"hero","energy":20},{"id":"enemy","hp":840}]}},
            skip(),
            {"point":"insert","choice":{"kind":"skip"},
             "expect_after":{"elapsed_av":120,"cycle_remaining_av":0,"remaining_av":0}}
        ],
        "expect_result":{"outcome":"defeat","termination":"av_budget_exhausted","penalty_av":0,
                         "actions":2,"waves_cleared":0,"state":{"units":[{"id":"enemy","alive":true,"hp":840}]}}
    });
    let report = run(&file, script.clone()).unwrap();
    assert_eq!(report["steps"][4]["observation"]["now"], 100.0);
    assert_eq!(report["elapsed_av"], 120.0);
    let mut wrong_after_ultimate = script.clone();
    wrong_after_ultimate["steps"][0]["expect_after"]["units"][0]["energy"] = json!(20);
    let error = run(&file, wrong_after_ultimate).unwrap_err();
    assert_eq!(error, "script step 0, decision 0, state after: unit \"hero\".energy mismatch: expected 20.0 (abs_tol 0.0), got 0.0");
    let mut wrong_terminal_step = script;
    wrong_terminal_step["steps"][4]["expect_after"] = json!({"elapsed_av":100,"remaining_av":20});
    assert!(run(&file, wrong_terminal_step).unwrap_err().contains(
        "script step 4, decision 4, state after: elapsed_av mismatch: expected 100.0 (abs_tol 0.0), got 120.0"
    ));
}

#[test]
fn zero_step_opening_finish_and_total_vs_current_budget_are_distinct() {
    let mut input = scenario(vec![unit("hero", 100.0)], vec![vec![unit("enemy", 1.0)]]);
    input.max_av = 150.0;
    input.mechanics.cycles = vec![50.0, 100.0];
    input.mechanics.triggers.push(Trigger {
        owner: "hero".into(),
        event: BattleEvent::BattleStart,
        subject: None,
        action: None,
        uses: 1,
        expires_on: None,
        duration: 0,
        effects: vec![damage(1000.0, Target::AllEnemies)],
    });
    let file = file(&input);
    let checks = json!({
        "schema_version":1, "steps":[],
        "expect_opening":{"elapsed_av":0,"cycle_remaining_av":50,"remaining_av":150,
                          "units":[{"id":"enemy","alive":false,"hp":0}]},
        "expect_result":{"outcome":"victory","termination":"enemies_defeated",
                         "waves_cleared":1,"actions":0,"state":{"remaining_av":150}}
    });
    assert_eq!(run(&file, checks.clone()).unwrap()["steps"], json!([]));
    let mut wrong_budget = checks.clone();
    wrong_budget["expect_opening"]["cycle_remaining_av"] = json!(150);
    assert!(run(&file, wrong_budget)
        .unwrap_err()
        .contains("opening state: cycle_remaining_av mismatch"));
    let mut wrong_result = checks;
    wrong_result["expect_result"]["termination"] = json!("av_budget_exhausted");
    assert!(run(&file, wrong_result)
        .unwrap_err()
        .starts_with("result: termination mismatch"));
}

#[test]
fn exact_numbers_and_field_tolerance_keep_the_raw_hp_in_the_report() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action(
        "strike",
        vec![
            damage(10.75, Target::AbilityTarget),
            Effect::Toughness {
                amount: 3.0,
                target: Target::AbilityTarget,
            },
            Effect::Shield {
                scale: Scale::Attack,
                ratio: 0.0,
                flat: 10.0,
                max_total: 100.0,
                duration_turns: 2,
                target: Target::SelfUnit,
            },
        ],
    )];
    let mut input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    input.max_actions = 1;
    input.waves[0][0].initial_hp = 100.0;
    input.mechanics.toughness.insert("enemy".into(), 10.0);
    let file = file(&input);
    let checks = json!({"schema_version":1,"steps":[
        skip(),
        {"point":"normal_turn","choice":{"kind":"action","actor":"hero","action":"strike"},
         "expect_after":{"units":[{"id":"enemy","hp":{"value":89.5,"abs_tol":0.25},"toughness":7},
                                  {"id":"hero","shield":10}]}},
    ]});
    let report = run(&file, checks.clone()).unwrap();
    assert_eq!(report["units"][1]["hp"], 89.25);
    let mut exact = checks.clone();
    exact["steps"][1]["expect_after"]["units"][0]["hp"] = json!(89.25);
    assert!(run(&file, exact).is_ok());
    for (tolerance, expected) in [(0.24, "hp mismatch"), (-0.25, "nonnegative finite abs_tol")] {
        let mut invalid = checks.clone();
        invalid["steps"][1]["expect_after"]["units"][0]["hp"]["abs_tol"] = json!(tolerance);
        let error = run(&file, invalid).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }

    let mut invalid_events = checks;
    invalid_events["steps"][1]["expect"] = json!([{"offset":1000,"kind":"damage","actor":"hero"}]);
    invalid_events["steps"][1]["expect_after"]["units"][0]["hp"] = json!(100);
    assert!(run(&file, invalid_events)
        .unwrap_err()
        .contains("missing event at offset 1000"));
}

#[test]
fn observes_the_new_wave_and_retains_dead_units_at_the_same_av() {
    let file = ScenarioFile::from_slice(include_bytes!("../examples/team-waves.json")).unwrap();
    let mut script: Value =
        serde_json::from_slice(include_bytes!("../examples/team-waves-operations.json")).unwrap();
    script["steps"][22]["expect_after"] = json!({
        "wave":2,
        "elapsed_av":{"value":272.73,"abs_tol":0.01},
        "units":[{"id":"wave1_b","hp":0,"alive":false},{"id":"wave2_boss","hp":1500,"alive":true}]
    });
    let report = run(&file, script.clone()).unwrap();
    assert_eq!(
        report["steps"][22]["observation"]["now"],
        report["steps"][23]["observation"]["now"]
    );
    assert_eq!(report["steps"][22]["wave"], 1);
    script["steps"][22]["expect_after"]["wave"] = json!(1);
    assert!(run(&file, script).unwrap_err().contains("script step 22"));
}

#[test]
fn missing_ambiguous_and_duplicate_units_fail_instead_of_matching_first() {
    let mut input = scenario(
        vec![unit("hero", 100.0)],
        vec![vec![unit("same", 1.0), unit("same", 1.0)]],
    );
    input.waves.push(vec![unit("future", 1.0)]);
    let file = file(&input);
    for (units, expected) in [
        (
            json!([{"id":"future"}]),
            "unit \"future\" is not in this snapshot",
        ),
        (
            json!([{"id":"same","hp":1000}]),
            "unit \"same\" is ambiguous",
        ),
        (
            json!([{"id":"hero"},{"id":"hero"}]),
            "duplicate expected unit ID \"hero\"",
        ),
    ] {
        let script = json!({"schema_version":1,"steps":[],"expect_opening":{"units":units}});
        assert!(run(&file, script).unwrap_err().contains(expected));
    }
}

#[test]
fn checkpoint_fields_must_use_named_objects_and_non_null_observations() {
    for raw in [
        r#"{"schema_version":1,"steps":[],"expect_opening":null}"#,
        r#"{"schema_version":1,"steps":[],"expect_result":{"termination":null}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"elapsed_av":null}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"elapsed_av":0,"elapsed_av":1}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"units":[{"id":"hero","hp":null}]}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"units":[{"id":"hero","hp":{"value":1}}]}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":[]}"#,
        r#"{"schema_version":1,"steps":[],"expect_result":[]}"#,
        r#"{"schema_version":1,"steps":[],"expect_result":{"state":[]}}"#,
        r#"{"schema_version":1,"steps":[],"expect_result":{"outcome":{"victory":null}}}"#,
        r#"{"schema_version":1,"steps":[],"expect_result":{"termination":{"enemies_defeated":null}}}"#,
        r#"{"schema_version":1,"steps":[{"point":"insert","choice":{"kind":"skip"},"expect_after":[]}]}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"units":[["hero"]]}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"units":[{"id":"hero","hp":[1,0]}]}}"#,
        r#"{"schema_version":1,"steps":[],"expect_opening":{"remaining_av":{"value":1,"value":0,"abs_tol":1}}}"#,
    ] {
        assert!(serde_json::from_str::<ReplayScript>(raw).is_err(), "{raw}");
    }
}
