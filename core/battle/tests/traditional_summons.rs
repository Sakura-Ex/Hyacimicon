mod common;

use battle_core::{
    decision::DecisionPoint,
    engine::{simulate, BattleSession, BattleStep},
    model::*,
};
use common::*;

fn opening_summon() -> Scenario {
    let mut input = scenario(vec![unit("owner", 100.0)], vec![vec![unit("enemy", 1.0)]]);
    input.max_av = 140.0;
    let mut strike = action("summon_strike", vec![damage(40.0, Target::EnemyLowestHp)]);
    strike.kind = ActionKind::FollowUp;
    input.mechanics.traditional_summons.push(TraditionalSummon {
        id: "summon".into(),
        owner: "owner".into(),
        stats: unit("unused", 80.0).stats,
        action: strike,
    });
    input
}

#[test]
fn opening_summon_runs_at_125_without_a_policy_choice() {
    let input = opening_summon();
    let mut session = BattleSession::new(&input, 42).unwrap();
    let mut manual_turns = vec![];
    while let BattleStep::Decision(decision) = session.advance().unwrap() {
        if decision.observation.now == 0.0 {
            let summon = decision
                .observation
                .axis
                .iter()
                .find(|entry| entry.id == "summon")
                .unwrap();
            close(summon.waiting_av, 125.0);
        }
        if decision.point != DecisionPoint::Insert {
            manual_turns.push(decision.observation.now);
        }
        let (id, choice) = (decision.id, decision.legal.first().cloned());
        session.submit(id, choice).unwrap();
    }
    let stepped = session.into_result().unwrap();
    let automatic = simulate(&input, 42).unwrap();
    assert_eq!(manual_turns, [100.0]);
    for result in [stepped, automatic] {
        assert_eq!(result.actions, 2);
        assert_eq!(result.termination, Termination::AvBudgetExhausted);
        close(result.elapsed_av, 140.0);
        close(state(&result, "enemy").hp, 960.0);
        close(state(&result, "summon").remaining_distance, 8800.0);
        assert_eq!(
            state(&result, "summon").summon_owner.as_deref(),
            Some("owner")
        );
    }
}

#[test]
fn summon_victory_stops_before_turn_end_can_kill_its_owner() {
    let mut input = opening_summon();
    input.max_av = 200.0;
    input.waves[0][0].initial_hp = 40.0;
    input.mechanics.triggers.push(Trigger {
        owner: "owner".into(),
        event: BattleEvent::TurnEnd,
        subject: Some("summon".into()),
        action: None,
        uses: 1,
        effects: vec![damage(1000.0, Target::SelfUnit)],
        expires_on: None,
        duration: 0,
    });

    let victory = simulate(&input, 42).unwrap();
    assert_eq!(victory.outcome, Outcome::Victory);
    assert_eq!(victory.termination, Termination::EnemiesDefeated);
    close(victory.elapsed_av, 125.0);
    close(victory.remaining_av, 75.0);
    close(state(&victory, "owner").hp, 1000.0);
    assert!(!state(&victory, "enemy").alive);

    // Leave the enemy alive to verify that the same turn-end listener is lethal.
    input.waves[0][0].initial_hp = 41.0;
    let defeat = simulate(&input, 42).unwrap();
    assert_eq!(defeat.outcome, Outcome::Defeat);
    assert_eq!(defeat.termination, Termination::AlliesDefeated);
    close(defeat.elapsed_av, 125.0);
    close(state(&defeat, "owner").hp, 0.0);
    close(state(&defeat, "enemy").hp, 1.0);
}

#[test]
fn owner_loss_dismisses_the_summon_but_immediate_revival_preserves_its_axis() {
    for revives in [false, true] {
        let mut input = opening_summon();
        input.allies.push(unit("witness", 1.0));
        if revives {
            input.allies[0].revival = Some(Revival {
                charges: 1,
                hp_fraction: 1.0,
                av_penalty: 0.0,
            });
        }
        let enemy = &mut input.waves[0][0];
        enemy.stats.speed = 10000.0 / 110.0;
        enemy.actions = vec![action("sweep", vec![damage(1000.0, Target::AllEnemies)])];

        // Keep the witness alive in both cases so time advances past AV 125.
        input.allies[1].stats.max_hp = 2000.0;
        input.allies[1].initial_hp = 2000.0;
        let result = simulate(&input, 42).unwrap();
        assert_eq!(result.termination, Termination::AvBudgetExhausted);
        assert_eq!(state(&result, "owner").alive, revives);
        let summon = state(&result, "summon");
        assert_eq!(summon.alive, revives);
        close(summon.hp, 1000.0);
        close(
            summon.remaining_distance,
            if revives { 8800.0 } else { 1200.0 },
        );
        close(
            state(&result, "enemy").hp,
            if revives { 960.0 } else { 1000.0 },
        );

        if !revives {
            input.allies.pop();
            let defeated = simulate(&input, 42).unwrap();
            assert_eq!(defeated.termination, Termination::AlliesDefeated);
            close(defeated.elapsed_av, 110.0);
        }
    }
}

#[test]
fn rejects_opening_summons_that_cannot_be_scheduled_unambiguously() {
    let input = opening_summon();
    let mut invalid = input.clone();
    invalid.mechanics.traditional_summons[0].id = "later_enemy".into();
    invalid.waves.push(vec![unit("later_enemy", 1.0)]);
    let error = simulate(&invalid, 42).unwrap_err();
    assert!(error.contains("duplicate traditional summon ID"));

    let mut invalid = input.clone();
    invalid.mechanics.traditional_summons[0].owner = "enemy".into();
    let error = simulate(&invalid, 42).unwrap_err();
    assert!(error.contains("unknown ally owner"));

    let mut invalid = input.clone();
    invalid.mechanics.traditional_summons[0].stats.speed = 0.0;
    let error = simulate(&invalid, 42).unwrap_err();
    assert!(error.contains("finite positive speed"));

    let mut invalid = input;
    invalid.mechanics.traditional_summons[0].action.sp_cost = 1;
    let error = simulate(&invalid, 42).unwrap_err();
    assert!(error.contains("resource-free follow-up"));
}

#[cfg(feature = "diagnostics")]
#[test]
fn diagnostic_fixture_matches_the_in_memory_input() {
    use battle_core::diagnostics::{simulate, ScenarioFile};

    let fixture = include_bytes!("../examples/opening-summon.json");
    let decoded = ScenarioFile::from_slice(fixture).unwrap();
    let from_file = simulate(&decoded.battle, 42, true).unwrap();
    let from_memory = simulate(&opening_summon(), 42, true).unwrap();
    assert_eq!(
        serde_json::to_value(&from_file).unwrap(),
        serde_json::to_value(&from_memory).unwrap()
    );

    let boundaries: Vec<_> = from_file
        .events
        .iter()
        .filter(|event| {
            event.actor == "summon"
                && matches!(
                    event.kind.as_str(),
                    "normal_turn_start" | "action_end" | "normal_turn_end"
                )
        })
        .map(|event| (event.kind.as_str(), event.av))
        .collect();
    assert_eq!(
        boundaries,
        [
            ("normal_turn_start", 125.0),
            ("action_end", 125.0),
            ("normal_turn_end", 125.0)
        ]
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn automatic_turn_at_deadline_refills_after_action_before_turn_end() {
    let mut input = opening_summon();
    input.max_av = 125.0;
    input.mechanics.unit_before_cycle = true;
    input.mechanics.queue_capacity = Some(1);
    input.waves[0].push(unit("reserve", 1.0));
    input.mechanics.traditional_summons[0].action.effects = vec![
        damage(1000.0, Target::AllEnemies),
        damage(500.0, Target::AllEnemies),
    ];
    input.mechanics.triggers.push(Trigger {
        owner: "summon".into(),
        event: BattleEvent::TurnEnd,
        subject: Some("summon".into()),
        action: None,
        uses: 1,
        effects: vec![damage(17.0, Target::AllEnemies)],
        expires_on: None,
        duration: 0,
    });
    let result = battle_core::diagnostics::simulate(&input, 42, true).unwrap();
    assert_eq!(result.actions, 2);
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    close(state(&result, "reserve").hp, 983.0);
    let events: Vec<_> = result
        .events
        .iter()
        .filter(|event| {
            event.kind == "queue_spawn"
                || event.kind == "cycle_end"
                || (event.actor == "summon"
                    && matches!(event.kind.as_str(), "action_end" | "normal_turn_end"))
        })
        .map(|event| event.kind.as_str())
        .collect();
    assert_eq!(
        events,
        ["action_end", "queue_spawn", "normal_turn_end", "cycle_end"]
    );
}

#[test]
fn generic_ally_buffs_and_random_enemy_attacks_exclude_the_summon() {
    let mut input = opening_summon();
    input.allies[0].actions = vec![action(
        "ward",
        vec![Effect::Shield {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 20.0,
            max_total: 20.0,
            duration_turns: 2,
            target: Target::AllAllies,
        }],
    )];
    let enemy = &mut input.waves[0][0];
    enemy.stats.speed = 10000.0 / 110.0;
    enemy.actions = vec![action("hit", vec![damage(10.0, Target::EnemyRandomTaunt)])];
    for seed in 0..32 {
        let result = simulate(&input, seed).unwrap();
        close(state(&result, "owner").shield, 10.0);
        close(state(&result, "summon").shield, 0.0);
        close(state(&result, "summon").hp, 1000.0);
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn pursued_hit_can_dismiss_the_summon_without_running_its_queued_action() {
    use battle_core::{
        elements::{DamageElement, Element, ElementProfile},
        expression::Expression,
    };
    let mut input = opening_summon();
    input.allies.push(unit("witness", 1.0));
    input.allies[1].stats.max_hp = 2000.0;
    input.allies[1].initial_hp = 2000.0;
    let profile = ElementProfile {
        damage_bonus: Element::ALL.into_iter().map(|e| (e, 0.0)).collect(),
        resistance: Element::ALL.into_iter().map(|e| (e, 0.0)).collect(),
    };
    for id in ["summon", "enemy"] {
        input
            .mechanics
            .element_profiles
            .insert(id.into(), profile.clone());
    }
    input.mechanics.traditional_summons[0].action.effects = vec![
        Effect::Request {
            kind: RequestKind::FollowUp,
            action: Some("summon_strike".into()),
            target: Target::SelfUnit,
        },
        Effect::ElementalDamage {
            scale: Scale::Attack,
            ratio: Expression::Constant { value: 0.0 },
            flat: Expression::Constant { value: 40.0 },
            element: DamageElement::Fixed {
                element: Element::Physical,
            },
            attack_type: AttackType::Pursued,
            can_crit: false,
            target: Target::AbilityTarget,
        },
    ];
    input.mechanics.triggers.push(Trigger {
        owner: "enemy".into(),
        event: BattleEvent::Hit,
        subject: Some("summon".into()),
        action: None,
        uses: 1,
        effects: vec![Effect::Conditional {
            condition: Condition::HitAttackType {
                attack_type: AttackType::Pursued,
            },
            then_effects: vec![damage(1000.0, Target::AllEnemies)],
            else_effects: vec![],
        }],
        expires_on: None,
        duration: 0,
    });

    let result = battle_core::diagnostics::simulate(&input, 42, true).unwrap();
    assert!(!state(&result, "owner").alive);
    assert!(!state(&result, "summon").alive);
    assert_eq!(result.actions, 2);
    let events: Vec<_> = result
        .events
        .iter()
        .filter(|e| {
            e.actor == "summon"
                && matches!(
                    e.kind.as_str(),
                    "request_queued"
                        | "summon_despawn"
                        | "action_end"
                        | "normal_turn_end"
                        | "request_invalid"
                )
        })
        .map(|e| e.kind.as_str())
        .collect();
    assert_eq!(
        events,
        [
            "request_queued",
            "summon_despawn",
            "action_end",
            "normal_turn_end"
        ]
    );
}

#[test]
fn all_enemies_condition_ignores_a_summons_self_modifier() {
    use battle_core::{
        expression::Expression,
        modifiers::{ModifierDefinition, StackMode},
    };
    let mut input = opening_summon();
    input.mechanics.modifier_definitions.insert(
        "mark".into(),
        ModifierDefinition {
            stack_mode: StackMode::ReplaceGlobal,
            max_stacks: 1,
            lifetime: None,
            properties: vec![],
        },
    );
    input.mechanics.triggers.push(Trigger {
        owner: "summon".into(),
        event: BattleEvent::BattleStart,
        subject: None,
        action: None,
        uses: 1,
        effects: vec![Effect::AddModifier {
            id: "mark".into(),
            stacks: Expression::Constant { value: 1.0 },
            target: Target::SelfUnit,
        }],
        expires_on: None,
        duration: 0,
    });
    let enemy = &mut input.waves[0][0];
    enemy.stats.speed = 10000.0 / 110.0;
    enemy.actions = vec![action(
        "marked_attack",
        vec![Effect::Conditional {
            condition: Condition::HasModifier {
                id: "mark".into(),
                target: Target::AllEnemies,
            },
            then_effects: vec![damage(90.0, Target::EnemyLowestHp)],
            else_effects: vec![damage(10.0, Target::EnemyLowestHp)],
        }],
    )];
    let result = simulate(&input, 42).unwrap();
    assert_eq!(state(&result, "summon").modifiers[0].id, "mark");
    close(state(&result, "owner").hp, 990.0);
}

#[cfg(feature = "diagnostics")]
#[test]
fn pending_action_runs_before_summon_turn_end_unless_action_limit_stops_it() {
    let mut input = opening_summon();
    let mut follow_up = action("owner_follow_up", vec![damage(20.0, Target::EnemyLowestHp)]);
    follow_up.kind = ActionKind::FollowUp;
    input.allies[0].actions.push(follow_up);
    input.mechanics.traditional_summons[0]
        .action
        .effects
        .push(Effect::Request {
            kind: RequestKind::FollowUp,
            action: Some("owner_follow_up".into()),
            target: Target::AllyLowestHp,
        });
    input.mechanics.triggers.push(Trigger {
        owner: "summon".into(),
        event: BattleEvent::TurnEnd,
        subject: Some("summon".into()),
        action: None,
        uses: 1,
        effects: vec![damage(1000.0, Target::EnemyLowestHp)],
        expires_on: None,
        duration: 0,
    });
    for (action_limit, expected_actions) in [(1000, 3), (2, 2)] {
        input.max_actions = action_limit;
        let result = battle_core::diagnostics::simulate(&input, 42, true).unwrap();
        let termination = if action_limit == 2 {
            close(state(&result, "enemy").hp, 960.0);
            Termination::ActionLimit
        } else {
            Termination::EnemiesDefeated
        };
        assert_eq!(result.termination, termination);
        assert_eq!(result.actions, expected_actions);
        close(result.elapsed_av, 125.0);
        let markers: Vec<_> = result
            .events
            .iter()
            .filter_map(|event| {
                if event.kind == "action" {
                    event.action.as_deref()
                } else if event.kind == "normal_turn_end" && event.actor == "summon" {
                    Some("summon_turn_end")
                } else if event.kind == "normal_turn_interrupted" && event.actor == "summon" {
                    Some("summon_turn_interrupted")
                } else {
                    None
                }
            })
            .collect();
        let expected = if action_limit == 2 {
            vec!["wait", "summon_strike", "summon_turn_interrupted"]
        } else {
            vec![
                "wait",
                "summon_strike",
                "owner_follow_up",
                "summon_turn_end",
            ]
        };
        assert_eq!(markers, expected);
    }
}

#[test]
fn an_active_summon_cannot_set_an_invalid_axis_speed() {
    for speed in [-80.0, 0.0, f64::INFINITY] {
        let mut input = opening_summon();
        input.mechanics.traditional_summons[0]
            .action
            .effects
            .push(Effect::Speed {
                value: speed,
                target: Target::SelfUnit,
            });
        let error = simulate(&input, 42).unwrap_err();
        assert_eq!(
            error,
            "traditional summon summon needs finite positive speed"
        );
    }
}

#[test]
fn departed_summon_keeps_its_last_modifier_state() {
    use battle_core::{
        expression::Expression,
        modifiers::{Lifetime, ModifierDefinition, StackMode, TickUnit},
    };
    let mut input = opening_summon();
    input.allies.push(unit("witness", 1.0));
    let enemy = &mut input.waves[0][0];
    enemy.stats.speed = 10000.0 / 110.0;
    enemy.actions = vec![action(
        "knockout",
        vec![damage(1000.0, Target::EnemyLowestHp)],
    )];

    input.mechanics.modifier_definitions.insert(
        "opening_mark".into(),
        ModifierDefinition {
            stack_mode: StackMode::ReplaceGlobal,
            max_stacks: 1,
            lifetime: Some(Lifetime {
                event: BattleEvent::CycleEnd,
                unit: TickUnit::Owner,
                ticks: 1,
            }),
            properties: vec![],
        },
    );
    input.mechanics.triggers.push(Trigger {
        owner: "summon".into(),
        event: BattleEvent::BattleStart,
        subject: None,
        action: None,
        uses: 1,
        effects: vec![Effect::AddModifier {
            id: "opening_mark".into(),
            stacks: Expression::Constant { value: 1.0 },
            target: Target::SelfUnit,
        }],
        expires_on: None,
        duration: 0,
    });
    let result = simulate(&input, 42).unwrap();
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    let summon = state(&result, "summon");
    assert!(!summon.alive);
    assert_eq!(summon.modifiers[0].remaining_ticks, Some(1));
}
