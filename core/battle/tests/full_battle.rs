//! Hand-computed queue/HP examples and one natural Anaxa vertical run.
mod common;
use battle_core::{
    batch::{run_batch, BatchRequest},
    engine::{simulate, simulate_cancellable},
    model::*,
    settlement::SettlementPolicy,
};
use common::*;

fn queued(count: usize, capacity: usize) -> Scenario {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("strike", vec![damage(1000.0, Target::AllEnemies)])];
    let enemies = (0..count)
        .map(|i| unit(&format!("e{i}"), 10.0))
        .collect::<Vec<_>>();
    let weights = enemies.iter().map(|u| (u.id.clone(), 1.0)).collect();
    let mut s = scenario(vec![hero], vec![enemies]);
    s.max_av = 500.0;
    s.mechanics.queue_capacity = Some(capacity);
    s.mechanics.settlement = Some(SettlementPolicy {
        name: "weighted_kills_flat_penalty_v1".into(),
        enemy_weights: weights,
        minimum_progress: 0.5,
        hp_before: 60.0,
        hp_cap: 100.0,
        failure_loss: 20.0,
        recovery: 3.0,
    });
    s
}

#[test]
fn refill_two_empty_slots_once_and_finish_full_queue() {
    let input = validate(queued(4, 2)).unwrap();
    let r = simulate(&input, 7, true).unwrap();
    assert_eq!(r.outcome, Outcome::Victory);
    close(r.elapsed_av, 200.0);
    close(r.remaining_av, 300.0);
    assert_eq!(r.actions, 2);
    let spawns = r
        .events
        .iter()
        .filter(|e| e.kind == "queue_spawn")
        .collect::<Vec<_>>();
    assert_eq!(spawns.len(), 2);
    assert_eq!((&*spawns[0].actor, spawns[0].amount), ("e2", Some(0.0)));
    assert_eq!((&*spawns[1].actor, spawns[1].amount), ("e3", Some(1.0)));
    close(spawns[0].av, 100.0);
    let last_action_end = r
        .events
        .iter()
        .position(|e| e.kind == "action_end")
        .unwrap();
    assert!(last_action_end < spawns[0].index as usize);
    let settled = r.settlement.unwrap();
    close(settled.progress, 1.0);
    assert_eq!(settled.defeated_ids, ["e0", "e1", "e2", "e3"]);
    assert_eq!(settled.hp_after, Some(63.0));
    assert_eq!(r.run_hp_loss, Some(0.0));
    assert!(r.queue.unwrap().remaining_ids.is_empty());
}

#[test]
fn timeout_progress_and_run_survival_are_separate_from_combat_victory() {
    let mut s = queued(3, 1);
    s.max_av = 250.0;
    let r = simulate(&validate(s.clone()).unwrap(), 0, false).unwrap();
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(r.termination, Termination::AvBudgetExhausted);
    close(r.remaining_av, 0.0);
    let hp = r.settlement.unwrap();
    close(hp.progress, 2.0 / 3.0);
    assert!(hp.minimum_progress_met);
    assert_eq!(hp.hp_after, Some(63.0));
    s.mechanics.settlement.as_mut().unwrap().minimum_progress = 0.75;
    let r = simulate(&validate(s).unwrap(), 0, false).unwrap();
    let hp = r.settlement.unwrap();
    assert!(!hp.minimum_progress_met);
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(hp.actual_loss, Some(20.0));
    assert_eq!(hp.hp_after, Some(43.0));
    assert_eq!(hp.run_alive, Some(true));
}

#[test]
fn threshold_equality_and_hp_caps_use_declared_order() {
    let mut s = queued(2, 1);
    s.max_av = 150.0;
    let p = s.mechanics.settlement.as_mut().unwrap();
    p.hp_before = 99.0;
    let r = simulate(&validate(s.clone()).unwrap(), 0, false).unwrap();
    let hp = r.settlement.unwrap();
    assert!(hp.minimum_progress_met);
    assert_eq!(hp.hp_after, Some(100.0));
    assert_eq!(hp.actual_recovery, Some(1.0));
    assert_eq!(hp.recovery_clipped, Some(2.0));
    let p = s.mechanics.settlement.as_mut().unwrap();
    p.minimum_progress = 1.0;
    p.hp_before = 5.0;
    p.recovery = 0.0;
    let r = simulate(&validate(s.clone()).unwrap(), 0, false).unwrap();
    let hp = r.settlement.unwrap();
    assert_eq!(hp.actual_loss, Some(5.0));
    assert_eq!(hp.loss_clipped, Some(15.0));
    assert_eq!(hp.hp_after, Some(0.0));
    assert_eq!(hp.run_alive, Some(false));
    s.mechanics.settlement.as_mut().unwrap().recovery = 3.0;
    let hp = simulate(&validate(s).unwrap(), 0, false)
        .unwrap()
        .settlement
        .unwrap();
    assert_eq!(hp.hp_after, Some(3.0)); // named policy permits post-loss recovery
}

#[test]
fn cancellation_and_action_limit_do_not_settle_a_partial_queue() {
    let mut s = queued(3, 1);
    let mut checks = 0;
    let r = simulate_cancellable(&validate(s.clone()).unwrap(), 0, true, &mut || {
        checks += 1;
        checks == 2
    })
    .unwrap();
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.outcome, Outcome::Incomplete);
    assert_eq!(r.actions, 1);
    assert_eq!(r.queue.unwrap().remaining_ids, ["e1", "e2"]);
    let hp = r.settlement.unwrap();
    close(hp.progress, 1.0 / 3.0);
    assert_eq!(hp.hp_after, None);
    assert_eq!(hp.status, "not_applied_incomplete");
    assert_eq!(r.run_hp_loss, None);
    s.max_actions = 1;
    let input = validate(s).unwrap();
    let r = simulate(&input, 0, false).unwrap();
    assert_eq!(r.termination, Termination::ActionLimit);
    assert_eq!(r.settlement.unwrap().hp_after, None);
    let b = run_batch(&input, &BatchRequest { trials: 2, seed: 0 }, &mut || false).unwrap();
    assert_eq!((b.completed, b.incomplete, b.settled_count), (0, 2, 0));
    assert_eq!(b.mean_hp_after, None);
    assert_eq!(b.win_rate, None);
}

#[test]
fn revival_keeps_slot_and_does_not_count_as_defeated_progress() {
    let mut s = queued(2, 1);
    s.waves[0][0].revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 40.0,
    });
    let r = simulate(&validate(s.clone()).unwrap(), 0, true).unwrap();
    close(r.elapsed_av, 300.0);
    close(r.penalty_av, 40.0);
    close(r.remaining_av, 160.0);
    let spawn = r.events.iter().find(|e| e.kind == "queue_spawn").unwrap();
    close(spawn.av, 200.0);
    assert_eq!(r.settlement.unwrap().defeated_ids, ["e0", "e1"]);
    s.max_av = 120.0;
    let r = simulate(&validate(s).unwrap(), 0, false).unwrap();
    close(r.penalty_av, 20.0);
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(r.termination, Termination::AvBudgetExhausted);
    assert!(r.settlement.unwrap().defeated_ids.is_empty());
    assert_eq!(r.queue.unwrap().remaining_ids, ["e1"]);
}

#[test]
fn final_hit_at_deadline_wins_but_waiting_queue_prevents_false_clear() {
    let mut s = queued(1, 1);
    s.max_av = 100.0;
    s.mechanics.unit_before_cycle = true;
    let r = simulate(&validate(s).unwrap(), 0, false).unwrap();
    assert_eq!(r.outcome, Outcome::Victory);
    close(r.remaining_av, 0.0);
    let mut s = queued(2, 1);
    s.max_av = 100.0;
    s.mechanics.unit_before_cycle = true;
    let r = simulate(&validate(s).unwrap(), 0, false).unwrap();
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(r.termination, Termination::AvBudgetExhausted);
    close(r.settlement.unwrap().progress, 0.5);
}

#[test]
fn malformed_queue_and_settlement_are_rejected_before_execution() {
    let mut s = queued(2, 1);
    s.mechanics.queue_capacity = Some(0);
    assert!(validate(s).unwrap_err().contains("capacity 1..32"));
    let mut s = queued(2, 1);
    s.mechanics
        .settlement
        .as_mut()
        .unwrap()
        .enemy_weights
        .remove("e1");
    assert!(validate(s)
        .unwrap_err()
        .contains("positive weight per enemy"));
    let mut s = queued(2, 1);
    s.mechanics.settlement.as_mut().unwrap().minimum_progress = 1.1;
    assert!(validate(s).unwrap_err().contains("minimum_progress"));
    let mut s = queued(2, 1);
    s.allies[0].actions[0].effects = vec![Effect::Conditional {
        condition: Condition::HasModifier {
            id: "misspelled".into(),
            target: Target::AbilityTarget,
        },
        then_effects: vec![],
        else_effects: vec![],
    }];
    assert!(validate(s)
        .unwrap_err()
        .contains("unknown condition modifier misspelled"));
}

#[test]
fn elimination_and_fractional_weight_full_progress_are_computed_from_state() {
    let mut s = queued(3, 3);
    s.waves[0].reverse(); // input order differs from the fixed map's summation order
    let policy = s.mechanics.settlement.as_mut().unwrap();
    policy.minimum_progress = 1.0;
    policy.enemy_weights.insert("e0".into(), 0.1);
    policy.enemy_weights.insert("e1".into(), 0.2);
    policy.enemy_weights.insert("e2".into(), 0.3);
    let hp = simulate(&validate(s).unwrap(), 0, false)
        .unwrap()
        .settlement
        .unwrap();
    assert_eq!(hp.progress, 1.0);
    assert!(hp.minimum_progress_met);

    let mut s = queued(1, 1);
    s.allies[0].stats.speed = 10.0;
    s.waves[0][0].stats.speed = 100.0;
    s.waves[0][0].actions = vec![action("fatal", vec![damage(1000.0, Target::AllEnemies)])];
    let r = simulate(&validate(s).unwrap(), 0, false).unwrap();
    assert_eq!(r.termination, Termination::AlliesDefeated);
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(r.settlement.unwrap().hp_after, Some(43.0));
}

#[test]
fn anaxa_natural_actions_and_same_seed_batch_replay() {
    let s: Scenario =
        serde_json::from_str(include_str!("../examples/anaxa-s2-full-battle.json")).unwrap();
    let input = validate(s).unwrap();
    let r = simulate(&input, 42, true).unwrap();
    assert_eq!(r.outcome, Outcome::Victory);
    assert_eq!(r.termination, Termination::EnemiesDefeated);
    assert!(r.remaining_av > 0.0);
    assert_eq!(r.settlement.as_ref().unwrap().hp_after, Some(63.0));
    for id in [
        "140501_basic",
        "140502_skill",
        "140503_ultimate",
        "anaxa_bonus_skill_approx",
    ] {
        assert!(
            r.events
                .iter()
                .any(|e| e.kind == "action" && e.action.as_deref() == Some(id)),
            "missing {id}"
        );
    }
    assert!(r
        .events
        .iter()
        .any(|e| e.kind == "damage" && e.attack_type == Some(AttackType::Pursued)));
    assert!(r
        .events
        .iter()
        .any(|e| e.kind == "weakness_count" && e.amount == Some(7.0)));
    assert!(state(&r, "anaxa").dynamic_values["GridFight_S2_Current_MagicLayer"] >= 5.0);
    let mut traced = serde_json::to_value(&r).unwrap();
    traced["events"] = serde_json::json!([]);
    assert_eq!(
        traced,
        serde_json::to_value(simulate(&input, 42, false).unwrap()).unwrap()
    );
    let b = run_batch(
        &input,
        &BatchRequest {
            trials: 4,
            seed: 42,
        },
        &mut || false,
    )
    .unwrap();
    assert_eq!(b.settled_count, 4);
    assert_eq!(b.run_hp_loss_unknown_count, 0);
    assert_eq!(b.mean_hp_after, Some(63.0));
    for trial in b.ledger {
        let single = simulate(&input, trial.seed, false).unwrap();
        assert_eq!(single.outcome, trial.outcome);
        assert_eq!(single.actions, trial.actions);
        assert_eq!(single.remaining_av, trial.remaining_av);
        assert_eq!(
            serde_json::to_value(single.settlement).unwrap(),
            serde_json::to_value(trial.settlement).unwrap()
        );
    }
}
