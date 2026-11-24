#![cfg(feature = "diagnostics")]
//! Replay recorded choices against independently calculated fixture results.

mod common;

use battle_core::{
    diagnostics::{
        replay::{replay, ReplayScript},
        ScenarioFile,
    },
    elements::DamageElement,
    expression::Expression,
    model::*,
};
use common::{action, damage, scenario, unit};
use serde_json::{json, Value};

fn run(input: &Scenario, script: Value, seed: u64) -> Result<Value, String> {
    let script: ReplayScript = serde_json::from_value(script).map_err(|e| e.to_string())?;
    let scenario = ScenarioFile::from_slice(&serde_json::to_vec(input).unwrap())?;
    Ok(serde_json::to_value(replay(&scenario, seed, &script)?).unwrap())
}

fn strike(amount: f64, target: Target, can_crit: bool) -> Effect {
    let mut effect = damage(amount, target);
    if let Effect::Damage {
        can_crit: enabled, ..
    } = &mut effect
    {
        *enabled = can_crit;
    }
    effect
}

fn input(effects: Vec<Effect>) -> Scenario {
    let mut hero = unit("hero", 100.0);
    hero.stats.crit_rate = 0.5;
    hero.actions = vec![action("strike", effects)];
    let mut input = scenario(
        vec![hero],
        vec![vec![unit("left", 1.0), unit("right", 1.0)]],
    );
    input.max_actions = 1;
    input.waves[0][0].initial_hp = 500.0;
    input
}

fn skip() -> Value {
    json!({"point":"insert", "choice":{"kind":"skip"}})
}

fn normal_script(target: Option<&str>, outcomes: Vec<Value>) -> Value {
    let mut choice = json!({"kind":"action", "actor":"hero", "action":"strike"});
    if let Some(target) = target {
        choice["target"] = json!(target);
    }
    json!({"schema_version":1, "steps":[skip(), {"point":"normal_turn", "choice":choice}],
           "random_outcomes":outcomes})
}

fn draw(step: Option<usize>, action: &str, result: Value) -> Value {
    json!({"step":step, "actor":"hero", "action":action, "result":result})
}

fn crit(step: Option<usize>, action: &str, target: &str, critical: bool) -> Value {
    draw(
        step,
        action,
        json!({"kind":"critical", "target":target, "critical":critical}),
    )
}

fn target(kind: &str, id: &str) -> Value {
    draw(Some(1), "strike", json!({"kind":kind,"target":id}))
}

fn damage_events(report: &Value) -> Vec<(&str, f64, bool)> {
    report["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "damage")
        .map(|e| {
            (
                e["target"].as_str().unwrap(),
                e["amount"].as_f64().unwrap(),
                e["critical"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn trigger(event: BattleEvent, effects: Vec<Effect>) -> Trigger {
    Trigger {
        owner: "hero".into(),
        event,
        subject: None,
        action: None,
        uses: 1,
        effects,
        expires_on: None,
        duration: 0,
    }
}

#[test]
fn chosen_primary_random_enemy_and_two_critical_outcomes_drive_damage() {
    let battle = input(vec![
        strike(100.0, Target::AbilityTarget, true),
        strike(100.0, Target::EnemyRandomUniform, true),
    ]);
    let outcomes = vec![
        crit(Some(1), "strike", "right", false),
        target("uniform_enemy", "left"),
        crit(Some(1), "strike", "left", true),
    ];
    for seed in [3, 42] {
        let report = run(
            &battle,
            normal_script(Some("right"), outcomes.clone()),
            seed,
        )
        .unwrap();
        assert_eq!(
            damage_events(&report),
            [("right", 100.0, false), ("left", 200.0, true)]
        );
        assert_eq!(report["random_outcomes"], json!(outcomes));
    }
    let bad = normal_script(Some("right"), outcomes[..1].to_vec());
    assert!(run(&battle, bad, 42)
        .unwrap_err()
        .contains("script step 1, decision 1: missing random outcome 1"));

    let mut wrong_step = outcomes.clone();
    wrong_step[1]["step"] = json!(2);
    let bad = normal_script(Some("right"), wrong_step);
    assert!(run(&battle, bad, 42)
        .unwrap_err()
        .contains("next record is for step Some(2)"));

    let mut wrong_kind = outcomes.clone();
    wrong_kind.swap(0, 1);
    assert!(run(&battle, normal_script(Some("right"), wrong_kind), 42)
        .unwrap_err()
        .contains("random outcome 0 mismatch"));

    let mut extra = outcomes.clone();
    extra.push(crit(Some(1), "strike", "left", false));
    assert!(run(&battle, normal_script(Some("right"), extra), 42)
        .unwrap_err()
        .contains("unused random outcome 3"));
    let mut wrong_source = outcomes;
    wrong_source[0]["actor"] = json!("other");
    assert!(run(&battle, normal_script(Some("right"), wrong_source), 42)
        .unwrap_err()
        .contains("random outcome 0 mismatch"));
}

#[test]
fn variable_critical_outcomes_require_unique_living_instance_ids() {
    let mut battle = input(vec![strike(100.0, Target::AllEnemies, true)]);
    battle.waves[0][1].id = "left".into();
    let outcomes = vec![
        crit(Some(1), "strike", "left", false),
        crit(Some(1), "strike", "left", true),
    ];
    let with_tape = normal_script(None, outcomes);
    let error = run(&battle, with_tape.clone(), 42).unwrap_err();
    assert!(
        error.contains("ambiguous recorded critical target \"left\""),
        "{error}"
    );

    let mut seeded = with_tape;
    seeded.as_object_mut().unwrap().remove("random_outcomes");
    assert_eq!(damage_events(&run(&battle, seeded, 42).unwrap()).len(), 2);

    // A defeated instance with the same name does not hide a single living target.
    battle.allies[0].actions[0]
        .effects
        .insert(0, strike(1000.0, Target::EnemyLowestHp, false));
    let script = normal_script(None, vec![crit(Some(1), "strike", "left", true)]);
    let report = run(&battle, script, 42).unwrap();
    assert_eq!(
        damage_events(&report),
        [("left", 1000.0, false), ("left", 200.0, true)]
    );
}

#[test]
fn variable_target_draws_require_a_unique_living_actor_id() {
    let mut battle = input(vec![strike(100.0, Target::EnemyRandomUniform, false)]);
    battle.allies.push(unit("hero", 1.0));
    let script = normal_script(None, vec![target("uniform_enemy", "left")]);
    let error = run(&battle, script, 42).unwrap_err();
    assert!(
        error.contains("ambiguous recorded actor \"hero\""),
        "{error}"
    );
}

#[test]
fn opening_can_consume_randomness_and_finish_without_a_decision() {
    let mut battle = input(vec![]);
    battle.waves[0].truncate(1);
    battle.waves[0][0].initial_hp = 150.0;
    battle.mechanics.triggers.push(trigger(
        BattleEvent::BattleStart,
        vec![strike(100.0, Target::AllEnemies, true)],
    ));
    let observed = crit(None, "event", "left", true);
    let report = run(
        &battle,
        json!({"schema_version":1, "steps":[], "random_outcomes":[observed.clone()]}),
        42,
    )
    .unwrap();
    assert_eq!(report["outcome"], "victory");
    assert_eq!(report["steps"], json!([]));
    assert_eq!(damage_events(&report), [("left", 200.0, true)]);
    assert!(run(
        &battle,
        json!({"schema_version":1,"steps":[],"random_outcomes":[]}),
        42
    )
    .unwrap_err()
    .contains("opening: missing random outcome 0"));
    assert!(run(
        &battle,
        json!({"schema_version":1,"steps":[],"random_outcomes":[observed.clone(), observed]}),
        42
    )
    .unwrap_err()
    .contains("opening: unused random outcome 1"));
}

#[test]
fn final_advance_can_consume_a_draw_with_no_following_decision() {
    let mut battle = input(vec![]);
    battle.max_av = 40.0;
    battle.mechanics.triggers.push(trigger(
        BattleEvent::CycleEnd,
        vec![strike(100.0, Target::EnemyLowestHp, true)],
    ));
    let observed = crit(Some(0), "event", "left", true);
    let report = run(
        &battle,
        json!({"schema_version":1,"steps":[skip()],"random_outcomes":[observed]}),
        42,
    )
    .unwrap();
    assert_eq!(report["termination"], "av_budget_exhausted");
    assert_eq!(damage_events(&report), [("left", 200.0, true)]);
    assert!(
        report["steps"][0]["event_after_submit"].as_u64()
            < report["steps"][0]["event_end"].as_u64()
    );
}

#[test]
fn an_insert_can_draw_during_a_later_step_than_its_submission() {
    let mut battle = input(vec![]);
    let hero = &mut battle.allies[0];
    hero.initial_energy = 20.0;
    hero.actions.push(Action {
        kind: ActionKind::FollowUp,
        ..action("prepared", vec![])
    });
    hero.actions.push(Action {
        kind: ActionKind::Ultimate,
        energy_cost: 20.0,
        ..action("ultimate", vec![strike(100.0, Target::AbilityTarget, true)])
    });
    battle.max_actions = 2;
    battle.mechanics.triggers.push(trigger(
        BattleEvent::BattleStart,
        vec![Effect::Request {
            kind: RequestKind::FollowUp,
            action: Some("prepared".into()),
            target: Target::SelfUnit,
        }],
    ));
    let mut script = json!({
        "schema_version":1, "steps":[
            {"point":"insert","choice":{"kind":"action","actor":"hero","action":"ultimate","target":"right"}},
            skip()
        ], "random_outcomes":[crit(Some(1),"ultimate","right",true)]
    });
    let report = run(&battle, script.clone(), 42).unwrap();
    assert_eq!(damage_events(&report), [("right", 200.0, true)]);
    script["random_outcomes"][0]["step"] = json!(0);
    assert!(run(&battle, script, 42)
        .unwrap_err()
        .contains("script step 0, decision 0: unused random outcome 0"));
}

#[test]
fn weighted_targets_require_positive_weights_and_live_instances() {
    let mut battle = input(vec![strike(100.0, Target::EnemyRandomTaunt, false)]);
    battle.waves[0][0].stats.taunt = 0.0;
    battle.waves[0].push(unit("third", 1.0));
    assert!(run(
        &battle,
        normal_script(None, vec![target("taunt_enemy", "left")]),
        42
    )
    .unwrap_err()
    .contains("illegal taunt_enemy target"));
    let report = run(
        &battle,
        normal_script(None, vec![target("taunt_enemy", "third")]),
        42,
    )
    .unwrap();
    assert_eq!(damage_events(&report), [("third", 100.0, false)]);

    battle.allies[0].actions[0].effects = vec![
        strike(600.0, Target::EnemyRandomUniform, false),
        strike(100.0, Target::EnemyRandomUniform, false),
    ];
    assert!(run(
        &battle,
        normal_script(
            None,
            vec![
                target("uniform_enemy", "left"),
                target("uniform_enemy", "left")
            ]
        ),
        42
    )
    .unwrap_err()
    .contains("illegal uniform_enemy target"));
    let report = run(
        &battle,
        normal_script(
            None,
            vec![
                target("uniform_enemy", "left"),
                target("uniform_enemy", "right"),
            ],
        ),
        42,
    )
    .unwrap();
    assert_eq!(
        damage_events(&report),
        [("left", 600.0, false), ("right", 100.0, false)]
    );
}

#[test]
fn retarget_draw_requires_an_enemy_that_this_action_has_already_hit() {
    let mut battle = input(vec![
        strike(100.0, Target::AbilityTarget, false),
        strike(100.0, Target::EnemyLowestHp, false),
        Effect::RetargetAttackRandom {
            effects: vec![strike(50.0, Target::ContextTarget, false)],
            empty_effects: vec![],
        },
    ]);
    battle.waves[0].push(unit("decoy", 1.0));
    assert!(run(
        &battle,
        normal_script(Some("right"), vec![target("hit_retarget", "decoy")]),
        42
    )
    .unwrap_err()
    .contains("illegal hit_retarget target"));
    let report = run(
        &battle,
        normal_script(Some("right"), vec![target("hit_retarget", "left")]),
        42,
    )
    .unwrap();
    assert_eq!(
        damage_events(&report),
        [
            ("right", 100.0, false),
            ("left", 100.0, false),
            ("left", 50.0, false)
        ]
    );
}

#[test]
fn deterministic_branches_are_inferred_and_unsupported_randomness_is_visible() {
    let mut battle = input(vec![strike(100.0, Target::EnemyRandomUniform, true)]);
    battle.waves[0].truncate(1);
    for (rate, expected) in [(0.0, 100.0), (1.0, 200.0)] {
        battle.allies[0].stats.crit_rate = rate;
        let report = run(&battle, normal_script(None, vec![]), 42).unwrap();
        assert_eq!(damage_events(&report), [("left", expected, rate == 1.0)]);
        assert!(run(
            &battle,
            normal_script(None, vec![crit(Some(1), "strike", "left", rate == 0.0)]),
            42
        )
        .unwrap_err()
        .contains("unused random outcome 0"));
    }
    battle.allies[0].actions[0].effects = vec![Effect::ElementalDamage {
        scale: Scale::Attack,
        ratio: Expression::Constant { value: 1.0 },
        flat: Expression::Constant { value: 0.0 },
        element: DamageElement::RandomAll,
        attack_type: AttackType::Direct,
        can_crit: false,
        target: Target::EnemyLowestHp,
    }];
    assert!(run(&battle, normal_script(None, vec![]), 42)
        .unwrap_err()
        .contains(
            "script step 1, decision 1: recorded replay does not yet support random elements"
        ));
    assert!(run(
        &battle,
        json!({"schema_version":1,"steps":[],"random_outcomes":null}),
        42
    )
    .is_err());
}
