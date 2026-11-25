#![cfg(feature = "diagnostics")]
//! Exercise explicit enemy instances at the live session and replay entry points.

mod common;

use battle_core::{
    decision::{Decision, DecisionPoint},
    diagnostics::{
        replay::{replay, ReplayScript},
        ScenarioFile,
    },
    engine::{BattleSession, BattleStep},
    model::*,
};
use common::{action, damage, scenario, state, unit};
use serde_json::{json, to_value};

fn next(session: &mut BattleSession<'_>) -> Decision {
    match session.advance().unwrap() {
        BattleStep::Decision(decision) => decision.clone(),
        BattleStep::Finished(_) => panic!("expected a battle decision"),
    }
}

fn next_turn(session: &mut BattleSession<'_>) -> Decision {
    loop {
        let decision = next(session);
        if decision.point != DecisionPoint::Insert {
            return decision;
        }
        session.submit(decision.id, None).unwrap();
    }
}

fn finish_without_inserts(mut session: BattleSession<'_>) -> BattleResult {
    while let BattleStep::Decision(decision) = session.advance().unwrap() {
        let choice = if decision.point == DecisionPoint::Insert {
            None
        } else {
            decision.legal.first().cloned()
        };
        let id = decision.id;
        session.submit(id, choice).unwrap();
    }
    session.into_result().unwrap()
}

fn two_enemies(hero: Unit) -> Scenario {
    let mut low = unit("low", 1.0);
    low.initial_hp = 50.0;
    let high = unit("high", 1.0);
    scenario(vec![hero], vec![vec![low, high]])
}

fn ultimate() -> Action {
    Action {
        kind: ActionKind::Ultimate,
        energy_cost: 20.0,
        ..action("ultimate", vec![damage(100.0, Target::AbilityTarget)])
    }
}

#[test]
fn ordinary_and_extra_turns_bind_the_explicit_enemy_after_invalid_submissions() {
    let mut hero = unit("hero", 100.0);
    hero.stats.crit_rate = 0.5;
    let mut hit = damage(100.0, Target::AbilityTarget);
    if let Effect::Damage { can_crit, .. } = &mut hit {
        *can_crit = true;
    }
    hero.actions = vec![action(
        "strike",
        vec![
            hit,
            Effect::Request {
                kind: RequestKind::ExtraTurn,
                action: None,
                target: Target::SelfUnit,
            },
        ],
    )];
    let mut input = two_enemies(hero);
    input.max_actions = 2;
    input.waves.push(vec![unit("future", 1.0)]);

    let run = |reject_first: bool| {
        let mut battle = BattleSession::with_trace(&input, 42, true).unwrap();
        let ordinary = next_turn(&mut battle);
        assert_eq!(ordinary.point, DecisionPoint::NormalTurn);
        let op = ordinary.legal[0].clone();
        assert_eq!(
            battle.legal_primary_targets(ordinary.id, &op).unwrap(),
            ["high", "low"]
        );
        if reject_first {
            let original = to_value(&ordinary).unwrap();
            for invalid in ["hero", "missing", "future"] {
                let error = battle
                    .submit_targeted(ordinary.id, op.clone(), invalid)
                    .unwrap_err();
                assert!(error.contains("not an active living enemy"));
                assert_eq!(to_value(next(&mut battle)).unwrap(), original);
            }
            assert_eq!(
                battle
                    .submit_targeted(ordinary.id - 1, op.clone(), "high")
                    .unwrap_err(),
                "stale decision id"
            );
            assert_eq!(to_value(next(&mut battle)).unwrap(), original);
        }
        battle.submit_targeted(ordinary.id, op, "high").unwrap();
        let extra = next_turn(&mut battle);
        assert_eq!(extra.point, DecisionPoint::ExtraTurn);
        battle
            .submit_targeted(extra.id, extra.legal[0].clone(), "high")
            .unwrap();
        finish_without_inserts(battle)
    };

    let chosen = run(true);
    assert_eq!(to_value(&chosen).unwrap(), to_value(run(false)).unwrap());
    assert_eq!(state(&chosen, "low").hp, 50.0);
    assert!(state(&chosen, "high").hp < 1000.0);
    assert_eq!(
        chosen
            .events
            .iter()
            .filter(|e| e.kind == "damage" && e.target.as_deref() == Some("high"))
            .count(),
        2
    );
}

#[test]
fn queued_ultimate_binds_high_enemy_and_never_retargets_a_later_hit() {
    let mut hero = unit("hero", 100.0);
    hero.initial_energy = 20.0;
    hero.actions.push(Action {
        effects: vec![
            damage(2000.0, Target::AbilityTarget),
            damage(20.0, Target::AbilityTarget),
        ],
        ..ultimate()
    });
    let mut input = two_enemies(hero);
    input.max_av = 20.0;
    let mut battle = BattleSession::with_trace(&input, 42, true).unwrap();
    let insert = next(&mut battle);
    assert_eq!(insert.point, DecisionPoint::Insert);
    battle
        .submit_targeted(insert.id, insert.legal[0].clone(), "high")
        .unwrap();
    let result = finish_without_inserts(battle);
    assert_eq!(state(&result, "hero").energy, 0.0);
    assert_eq!(state(&result, "high").hp, 0.0);
    assert_eq!(state(&result, "low").hp, 50.0);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| e.kind == "damage")
            .map(|e| e.target.as_deref())
            .collect::<Vec<_>>(),
        [Some("high")]
    );
}

#[test]
fn invalidated_queued_ultimate_does_not_spend_energy_or_take_the_reused_slot() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![
        action(
            "prepare",
            vec![
                Effect::Energy {
                    amount: 20.0,
                    target: Target::SelfUnit,
                },
                Effect::Request {
                    kind: RequestKind::FollowUp,
                    action: Some("follow".into()),
                    target: Target::SelfUnit,
                },
            ],
        ),
        Action {
            kind: ActionKind::FollowUp,
            ..action("follow", vec![damage(1000.0, Target::EnemyLowestHp)])
        },
        ultimate(),
    ];
    let mut input = two_enemies(hero);
    input.waves[0].push(unit("reserve", 1.0));
    input.mechanics.queue_capacity = Some(2);
    input.max_av = 101.0;
    let mut battle = BattleSession::with_trace(&input, 42, true).unwrap();
    let ordinary = next_turn(&mut battle);
    battle
        .submit(ordinary.id, Some(ordinary.legal[0].clone()))
        .unwrap();
    let insert = next(&mut battle);
    battle
        .submit_targeted(insert.id, insert.legal[0].clone(), "low")
        .unwrap();
    let refill = next(&mut battle);
    assert!(refill
        .observation
        .units
        .iter()
        .any(|u| u.id == "reserve" && u.alive));
    battle.submit(refill.id, None).unwrap();
    let retry = next(&mut battle);
    assert_eq!(
        battle
            .legal_primary_targets(retry.id, &retry.legal[0])
            .unwrap(),
        ["high", "reserve"]
    );
    assert!(battle
        .submit_targeted(retry.id, retry.legal[0].clone(), "low")
        .is_err());
    let result = finish_without_inserts(battle);
    assert_eq!(state(&result, "hero").energy, 20.0);
    assert_eq!(state(&result, "reserve").hp, 1000.0);
    assert_eq!(state(&result, "high").hp, 1000.0);
    assert!(!result
        .events
        .iter()
        .any(|e| e.kind == "ultimate_turn_start"));
    assert!(result
        .events
        .iter()
        .any(|e| e.kind == "request_target_invalid" && e.target.as_deref() == Some("low")));
}

#[test]
fn target_removed_during_ultimate_turn_start_keeps_entry_payment_without_a_redirect() {
    let mut hero = unit("hero", 100.0);
    hero.initial_energy = 20.0;
    hero.actions.push(ultimate());
    let mut input = two_enemies(hero);
    input.max_av = 20.0;
    input.mechanics.triggers.push(Trigger {
        owner: "hero".into(),
        subject: Some("hero".into()),
        event: BattleEvent::TurnStart,
        action: None,
        uses: 1,
        expires_on: None,
        duration: 0,
        effects: vec![damage(1000.0, Target::EnemyLowestHp)],
    });
    let mut battle = BattleSession::with_trace(&input, 42, true).unwrap();
    let insert = next(&mut battle);
    battle
        .submit_targeted(insert.id, insert.legal[0].clone(), "low")
        .unwrap();
    let result = finish_without_inserts(battle);
    assert_eq!(state(&result, "hero").energy, 0.0);
    assert_eq!(state(&result, "low").hp, 0.0);
    assert_eq!(state(&result, "high").hp, 1000.0);
    assert!(result
        .events
        .iter()
        .any(|e| e.kind == "request_target_invalid"));
    assert!(result.events.iter().any(|e| e.kind == "extra_turn_end"));
    assert!(!result
        .events
        .iter()
        .any(|e| e.action.as_deref() == Some("ultimate") && e.kind == "action"));
}

#[test]
fn rejects_ambiguous_ids_and_actions_without_an_explicit_bound_hit() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![
        action("strike", vec![damage(10.0, Target::AbilityTarget)]),
        action("aoe", vec![damage(10.0, Target::AllEnemies)]),
        action("random", vec![damage(10.0, Target::EnemyRandomUniform)]),
    ];
    let mut input = two_enemies(hero);
    input.waves[0].push(unit("high", 1.0));
    let mut battle = BattleSession::new(&input, 42).unwrap();
    let decision = next_turn(&mut battle);
    let op = decision.legal[0].clone();
    assert_eq!(
        battle.legal_primary_targets(decision.id, &op).unwrap(),
        ["low"]
    );
    assert_eq!(
        battle.submit_targeted(decision.id, op, "high").unwrap_err(),
        "ambiguous primary target \"high\""
    );
    for op in &decision.legal[1..] {
        assert_eq!(
            battle
                .submit_targeted(decision.id, op.clone(), "low")
                .unwrap_err(),
            "action does not accept a primary enemy target"
        );
    }
    assert_eq!(
        to_value(next(&mut battle)).unwrap(),
        to_value(decision).unwrap()
    );
}

#[test]
fn primary_target_after_wave_change_resolves_past_the_hidden_opening_actor() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("strike", vec![damage(100.0, Target::AbilityTarget)])];
    let mut input = two_enemies(hero);
    let second_wave = input.waves.pop().unwrap();
    let mut first = unit("first", 1.0);
    first.initial_hp = 1.0;
    input.waves = vec![vec![first], second_wave];
    input.max_actions = 2;
    input.mechanics.opening_battle_event = Some(OpeningBattleEvent {
        id: "train-source".into(),
        stats: unit("unused", 999.0).stats,
        action: Action {
            kind: ActionKind::FollowUp,
            ..action("pass", vec![damage(100.0, Target::AllEnemies)])
        },
    });
    let mut battle = BattleSession::with_trace(&input, 42, true).unwrap();
    let turn = next_turn(&mut battle);
    assert_eq!(turn.observation.units.len(), 4);
    assert_eq!(
        battle
            .legal_primary_targets(turn.id, &turn.legal[0])
            .unwrap(),
        ["high", "low"]
    );
    battle
        .submit_targeted(turn.id, turn.legal[0].clone(), "high")
        .unwrap();
    let result = finish_without_inserts(battle);
    assert_eq!(result.waves_cleared, 1);
    assert_eq!(state(&result, "low").hp, 50.0);
    assert_eq!(state(&result, "high").hp, 900.0);
}

#[test]
fn replay_names_the_selected_target_and_matches_the_live_session() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("strike", vec![damage(100.0, Target::AbilityTarget)])];
    let mut input = two_enemies(hero);
    input.max_actions = 1;
    let file = ScenarioFile::from_slice(&serde_json::to_vec(&input).unwrap()).unwrap();
    let script: ReplayScript = serde_json::from_value(json!({
        "schema_version": 1,
        "steps": [
            {"point":"insert", "choice":{"kind":"skip"}},
            {"point":"normal_turn", "choice":{"kind":"action","actor":"hero","action":"strike","target":"high"}}
        ]
    })).unwrap();
    let report = to_value(replay(&file, 42, &script).unwrap()).unwrap();
    let mut battle = BattleSession::with_trace(&file.battle, 42, true).unwrap();
    let turn = next_turn(&mut battle);
    battle
        .submit_targeted(turn.id, turn.legal[0].clone(), "high")
        .unwrap();
    let live = to_value(finish_without_inserts(battle)).unwrap();
    for (key, value) in live.as_object().unwrap() {
        assert_eq!(&report[key], value, "{key}");
    }
    assert_eq!(report["steps"][1]["choice"]["target"], "high");
    assert!(serde_json::from_value::<ReplayScript>(json!({
        "schema_version":1, "steps":[{"point":"insert","choice":{"kind":"skip","target":"high"}}]
    }))
    .is_err());
}
