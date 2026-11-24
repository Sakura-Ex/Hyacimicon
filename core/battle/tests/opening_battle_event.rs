//! Regression checks for a one-shot source outside the combatant roster.
mod common;

use battle_core::{
    decision::DecisionPoint,
    engine::{simulate, BattleSession, BattleStep},
    model::*,
};
use common::{action, close, damage, scenario, state, unit};

const TRAIN: &str = "62603";

fn with_train(allies: Vec<Unit>, waves: Vec<Vec<Unit>>) -> Scenario {
    let mut input = scenario(allies, waves);
    let mut strike = action(
        "train",
        [15.0, 15.0, 15.0, 15.0, 15.0, 25.0]
            .into_iter()
            .map(|amount| damage(amount, Target::AllEnemies))
            .collect(),
    );
    strike.kind = ActionKind::FollowUp;
    input.mechanics.opening_battle_event = Some(OpeningBattleEvent {
        id: TRAIN.into(),
        stats: unit("source", 999.0).stats,
        action: strike,
    });
    input
}

fn start_effect(owner: &str, effect: Effect) -> Trigger {
    Trigger {
        owner: owner.into(),
        event: BattleEvent::BattleStart,
        subject: None,
        action: None,
        uses: 1,
        expires_on: None,
        duration: 0,
        effects: vec![effect],
    }
}

#[test]
fn opening_event_is_one_action_outside_the_axis_and_direct_ally_factor() {
    let mut ally = unit("ally", 200.0);
    ally.actions = vec![action("basic", vec![damage(40.0, Target::EnemyLowestHp)])];
    let mut input = with_train(
        vec![ally],
        vec![vec![
            unit("left", 100.0),
            unit("middle", 99.0),
            unit("right", 98.0),
        ]],
    );
    input.max_actions = 1;
    input.mechanics.ally_damage_factor = 0.25;
    let gain = |amount| Effect::Energy {
        amount,
        target: Target::SelfUnit,
    };
    input.mechanics.triggers.push(Trigger {
        owner: "ally".into(),
        event: BattleEvent::Hit,
        subject: None,
        action: None,
        uses: 0,
        expires_on: None,
        duration: 0,
        effects: vec![Effect::Conditional {
            condition: Condition::All {
                conditions: vec![
                    Condition::HitAttackType {
                        attack_type: AttackType::Insert,
                    },
                    Condition::Not {
                        condition: Box::new(Condition::ActionKind {
                            kind: ActionKind::FollowUp,
                        }),
                    },
                ],
            },
            then_effects: vec![gain(1.0)],
            else_effects: vec![gain(2.0)],
        }],
    });
    let mut session = BattleSession::new(&input, 42).unwrap();
    let BattleStep::Decision(decision) = session.advance().unwrap() else {
        panic!("the opening insertion decision should precede the event action");
    };
    assert_eq!(decision.point, DecisionPoint::Insert);
    assert!(decision.legal.is_empty());
    assert_eq!(decision.observation.units.len(), 4);
    assert_eq!(decision.observation.axis.len(), 4);
    assert!(decision
        .observation
        .axis
        .iter()
        .all(|unit| unit.id != TRAIN));
    let id = decision.id;
    session.submit(id, None).unwrap();
    let BattleStep::Finished(result) = session.advance().unwrap() else {
        panic!("one complete action should reach the action limit");
    };
    assert_eq!(result.actions, 1);
    assert_eq!(result.termination, Termination::ActionLimit);
    close(result.elapsed_av, 0.0);
    assert_eq!(result.units.len(), 4);
    for id in ["left", "middle", "right"] {
        close(state(result, id).hp, 900.0);
    }
    close(state(result, "ally").energy, 18.0);

    input.max_actions = 2;
    let result = simulate(&input, 42).unwrap();
    close(state(&result, "left").hp, 890.0);
    close(state(&result, "middle").hp, 900.0);
    close(state(&result, "ally").energy, 20.0);
    close(result.elapsed_av, 50.0);
}

#[test]
fn it_cannot_attack_the_second_wave_after_opening_effects_clear_the_first() {
    let mut input = with_train(
        vec![unit("ally", 200.0)],
        vec![vec![unit("first", 100.0)], vec![unit("second", 99.0)]],
    );
    input.max_actions = 1;
    input
        .mechanics
        .triggers
        .push(start_effect("ally", damage(1000.0, Target::AllEnemies)));
    let result = simulate(&input, 42).unwrap();
    assert_eq!(result.waves_cleared, 1);
    close(state(&result, "second").hp, 1000.0);
    close(result.elapsed_av, 50.0);
}

#[test]
fn event_survives_the_first_ally_but_cannot_keep_the_team_in_battle() {
    let mut enemy = unit("enemy", 200.0);
    enemy.actions = vec![action("kill", vec![damage(1000.0, Target::AllEnemies)])];
    let mut input = with_train(
        vec![unit("first", 98.0), unit("last", 99.0)],
        vec![vec![enemy]],
    );
    input
        .mechanics
        .triggers
        .push(start_effect("first", damage(1000.0, Target::SelfUnit)));
    let result = simulate(&input, 42).unwrap();
    assert_eq!(result.termination, Termination::AlliesDefeated);
    assert_eq!(result.actions, 2);
    assert_eq!(result.units.len(), 3);
    close(state(&result, "enemy").hp, 900.0);
}

#[test]
fn duplicate_source_id_is_rejected_against_future_enemies() {
    let input = with_train(
        vec![unit("ally", 200.0)],
        vec![vec![unit("first", 100.0)], vec![unit(TRAIN, 99.0)]],
    );
    assert_eq!(
        simulate(&input, 42).unwrap_err(),
        "invalid or duplicate opening battle event ID: \"62603\""
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn opening_break_precedes_the_eighteen_insert_hits_of_one_event_action() {
    let mut input = with_train(
        vec![unit("ally", 200.0)],
        vec![vec![
            unit("left", 100.0),
            unit("middle", 99.0),
            unit("right", 98.0),
        ]],
    );
    input.max_actions = 1;
    for enemy in &mut input.waves[0] {
        enemy.stats.toughness_broken = false;
        input.mechanics.toughness.insert(enemy.id.clone(), 10.0);
    }
    input.mechanics.triggers.push(start_effect(
        "ally",
        Effect::Toughness {
            amount: 10.0,
            target: Target::AllEnemies,
        },
    ));
    let result = battle_core::diagnostics::simulate(&input, 42, true).unwrap();
    let count = |kind: &str| {
        result
            .events
            .iter()
            .filter(|event| event.kind == kind)
            .count()
    };
    assert_eq!(count("break"), 3);
    assert_eq!(count("action"), 1);
    assert_eq!(count("action_end"), 1);
    let queued = result
        .events
        .iter()
        .find(|event| event.kind == "request_queued")
        .unwrap();
    assert_eq!(queued.actor, TRAIN);
    assert!(result
        .events
        .iter()
        .filter(|event| event.kind == "break")
        .all(|event| event.index < queued.index));
    let damage: Vec<_> = result
        .events
        .iter()
        .filter(|event| event.kind == "damage")
        .collect();
    assert_eq!(damage.len(), 18);
    let end = result
        .events
        .iter()
        .find(|event| event.kind == "action_end")
        .unwrap();
    for (event, expected) in damage
        .chunks_exact(3)
        .zip([15.0, 15.0, 15.0, 15.0, 15.0, 25.0])
    {
        let targets = event
            .iter()
            .map(|hit| hit.target.as_deref().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(targets, ["left", "middle", "right"]);
        for hit in event {
            assert_eq!(hit.actor, TRAIN);
            assert_eq!(hit.attack_type, Some(AttackType::Insert));
            assert!(queued.index < hit.index && hit.index < end.index);
            close(hit.av, 0.0);
            close(hit.amount.unwrap(), expected);
        }
    }
}
