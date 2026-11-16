mod common;

use battle_core::model::{Effect, Outcome, Revival, Termination};
use common::*;
use run_core::*;
use sha2::{Digest, Sha256};

#[test]
fn automatic_settlement_avoids_caller_ids_without_resampling_on_retry() {
    let occupied = format!("auto-settle-{:x}", Sha256::digest(b"second"));
    let mut env = env();
    let first = env
        .sample(&request(&env, &occupied), &mut || false)
        .unwrap();
    env.settle(&settlement_request(
        &env,
        &first,
        &format!("{occupied}-1"),
        loss_rules(),
    ))
    .unwrap();
    let command = request(&env, "second");
    let result = env.step(&command, &mut || false).unwrap();
    assert_eq!(
        result.settlement.as_ref().unwrap().request_id,
        format!("{occupied}-2")
    );
    assert_eq!(result.observation.status, RunStatus::Completed);
    assert_eq!(result.observation.resources.gold, 17);
    assert_eq!(env.attempts().len(), 2);
    assert_eq!(env.settlements().len(), 2);
    let replay = env
        .step(&command, &mut || panic!("retry sampled again"))
        .unwrap();
    assert_eq!(json(&result), json(&replay));
    assert_eq!(env.transitions().len(), 2);
}

#[test]
fn actual_loss_continues_then_actual_victory_finishes_with_failure_experience() {
    let mut input = spec();
    input.provenance.data_snapshot = "run-global-snapshot".into();
    input.nodes[0].battle.provenance.data_snapshot = "first-battle-snapshot".into();
    input.nodes[1].battle.provenance.data_snapshot = "second-battle-snapshot".into();
    let mut env = RunEnv::new(input, "mixed-provenance-run".into(), 17).unwrap();
    assert_eq!(env.legal_actions(), vec![RunAction::FightCurrentNode]);
    let first = env.step(&request(&env, "first"), &mut || false).unwrap();
    assert_eq!(first.attempt.resolution.result.outcome, Outcome::Defeat);
    assert_eq!(
        first.attempt.resolution.diagnostic.failure_reason,
        Some(Termination::AlliesDefeated)
    );
    assert_eq!(first.attempt.resolution.result.actions, 1);
    assert_eq!(
        first.attempt.provenance.data_snapshot,
        "run-global-snapshot"
    );
    assert_eq!(
        first.attempt.battle_provenance.data_snapshot,
        "first-battle-snapshot"
    );
    assert_eq!(first.attempt.resolution.result.elapsed_av, 50.0);
    assert_eq!(
        first.attempt.resolution.diagnostic.success_remaining_av,
        None
    );
    assert_eq!(env.observe().resources.hp, 23.0);
    assert_eq!(env.observe().resources.gold, 12);
    assert_eq!(env.observe().node_index, 1);
    assert_eq!(env.observe().revision, 2);
    assert_eq!(env.observe().status, RunStatus::Ready);
    assert_eq!(env.transitions().len(), 1);
    assert!(!env.transitions()[0].terminated);
    assert!(!env.transitions()[0].truncated);
    assert_eq!(env.transitions()[0].reward, None);

    let second = env.step(&request(&env, "second"), &mut || false).unwrap();
    assert_eq!(second.attempt.resolution.result.outcome, Outcome::Victory);
    assert_eq!(
        second.attempt.provenance.data_snapshot,
        "run-global-snapshot"
    );
    assert_eq!(
        second.attempt.battle_provenance.data_snapshot,
        "second-battle-snapshot"
    );
    assert_eq!(
        second.attempt.resolution.diagnostic.success_remaining_av,
        Some(950.0)
    );
    assert_eq!(second.attempt.resolution.diagnostic.failure_reason, None);
    assert_eq!(env.observe().resources.hp, 23.0);
    assert_eq!(env.observe().resources.max_hp, 30.0);
    assert_eq!(env.observe().resources.gold, 17);
    assert_eq!(env.observe().resources.crystals, 0);
    assert_eq!(env.observe().revision, 4);
    assert_eq!(env.observe().status, RunStatus::Completed);
    assert!(env.legal_actions().is_empty());
    assert_eq!(env.attempts().len(), 2);
    assert_eq!(env.settlements().len(), 2);
    assert_eq!(env.transitions().len(), 2);
    assert!(env.transitions()[1].terminated);
    assert_eq!(
        env.transitions()[0].attempt.resolution.result.outcome,
        Outcome::Defeat
    );
    assert_eq!(env.transitions()[0].before.resources.hp, 30.0);
    assert_eq!(env.transitions()[0].after.resources.hp, 23.0);
    assert_eq!(env.transitions()[1].before, env.transitions()[0].after);
    assert!(env
        .transitions()
        .iter()
        .all(|transition| transition.reward.is_none()));
}

#[test]
fn missing_rules_retain_the_sample_and_later_settlement_does_not_resample() {
    let mut input = spec();
    input.nodes[0].settlement = None;
    let mut env = RunEnv::new(input, "missing-rule".into(), 17).unwrap();
    let command = request(&env, "one-sample");
    let first = env.step(&command, &mut || false).unwrap();
    assert_eq!(first.attempt.resolution.result.outcome, Outcome::Defeat);
    assert!(first.settlement.is_none());
    assert!(first.pending_reason.is_some());
    assert_eq!(env.observe().status, RunStatus::AwaitingSettlement);
    assert_eq!(env.observe().revision, 1);
    assert_eq!(env.observe().node_index, 0);
    assert_eq!(env.observe().resources.hp, 30.0);
    assert_eq!(env.observe().resources.gold, 10);
    assert!(env.transitions().is_empty());
    let second = env
        .step(&command, &mut || {
            panic!("cached sample must not poll cancellation")
        })
        .unwrap();
    assert_eq!(json(&second.attempt), json(&first.attempt));
    assert_eq!(env.attempts().len(), 1);
    let settlement = settlement_request(&env, &first.attempt, "supply-rule", loss_rules());
    let receipt = env.settle(&settlement).unwrap();
    assert_eq!(receipt.outcome, Outcome::Defeat);
    assert_eq!(env.attempts().len(), 1);
    assert_eq!(env.transitions().len(), 1);
    assert_eq!(
        env.transitions()[0].attempt.attempt_id,
        first.attempt.attempt_id
    );
    assert_eq!(env.observe().resources.hp, 23.0);
    assert_eq!(env.observe().resources.gold, 12);
}

#[test]
fn missing_outcome_branch_is_unknown_instead_of_zero_settlement() {
    let mut input = spec();
    input.nodes[0].settlement.as_mut().unwrap().defeat = None;
    let mut env = RunEnv::new(input, "unknown-loss-branch".into(), 17).unwrap();
    let step = env.step(&request(&env, "fight"), &mut || false).unwrap();
    assert_eq!(step.attempt.resolution.result.outcome, Outcome::Defeat);
    assert!(step.settlement.is_none());
    assert!(step.pending_reason.is_some());
    assert_eq!(env.observe().status, RunStatus::AwaitingSettlement);
    assert_eq!(env.observe().resources.hp, 30.0);
    assert_eq!(env.observe().resources.gold, 10);
    assert_eq!(env.attempts().len(), 1);
    assert!(env.settlements().is_empty());
}

#[test]
fn exact_sample_and_settlement_replays_are_idempotent_after_node_advance() {
    let mut env = env();
    let sample = request(&env, "sample-once");
    let attempt = env.sample(&sample, &mut || false).unwrap();
    let settlement = settlement_request(&env, &attempt, "settle-once", loss_rules());
    let receipt = env.settle(&settlement).unwrap();
    let after = json(&env.report());
    let replay = env
        .sample(&sample, &mut || panic!("replay cannot run the engine"))
        .unwrap();
    assert_eq!(json(&attempt), json(&replay));
    assert_eq!(json(&receipt), json(&env.settle(&settlement).unwrap()));
    assert_eq!(json(&env.report()), after);
    assert_eq!(env.transitions().len(), 1);
}

#[test]
fn conflicting_request_identity_and_stale_revisions_cannot_mutate_resources() {
    let (mut env, attempt) = pending();
    let before = json(&env.report());
    let mut stale = request(&env, "stale-request");
    stale.expected_revision = 0;
    let error = env
        .sample(&stale, &mut || panic!("stale input must not execute"))
        .unwrap_err();
    assert!(error.contains("revision"), "{error}");
    assert_eq!(json(&env.report()), before);

    let mut command = SampleRequest {
        request_id: attempt.request_id.clone(),
        node_id: attempt.node_id.clone(),
        expected_revision: attempt.before.revision,
        trace: true,
    };
    command.trace = false;
    let error = env
        .sample(&command, &mut || panic!("conflict must not execute"))
        .unwrap_err();
    assert!(error.contains("request"), "{error}");
    assert_eq!(json(&env.report()), before);

    let mut settlement = settlement_request(&env, &attempt, "stale-settlement", loss_rules());
    settlement.expected_revision = 0;
    let error = env.settle(&settlement).unwrap_err();
    assert!(error.contains("revision"), "{error}");
    assert_eq!(json(&env.report()), before);
}

#[test]
fn settled_attempt_cannot_be_repriced_with_another_rule_or_command() {
    let (mut env, attempt) = pending();
    let mut settlement = settlement_request(&env, &attempt, "first-settlement", loss_rules());
    env.settle(&settlement).unwrap();
    let after = json(&env.report());
    settlement.rules.defeat.as_mut().unwrap().gold = 100;
    let error = env.settle(&settlement).unwrap_err();
    assert!(error.contains("request"), "{error}");
    assert_eq!(json(&env.report()), after);
    settlement.request_id = "different-command".into();
    settlement.expected_revision = env.observe().revision;
    let error = env.settle(&settlement).unwrap_err();
    assert!(
        error.contains("settled") || error.contains("pending"),
        "{error}"
    );
    assert_eq!(json(&env.report()), after);
}

#[test]
fn cancellation_is_interrupted_without_loss_settlement_or_training_transition() {
    let mut env = env();
    let before = env.observe().resources;
    let command = request(&env, "cancelled");
    let step = env.step(&command, &mut || true).unwrap();
    assert_eq!(step.attempt.resolution.result.outcome, Outcome::Incomplete);
    assert_eq!(step.attempt.resolution.result.actions, 0);
    assert_eq!(
        step.attempt.resolution.diagnostic.incomplete_reason,
        Some(Termination::Cancelled)
    );
    assert_eq!(step.attempt.resolution.diagnostic.failure_reason, None);
    assert_eq!(
        step.attempt.resolution.diagnostic.success_remaining_av,
        None
    );
    assert!(step.settlement.is_none());
    assert_eq!(env.observe().status, RunStatus::Interrupted);
    assert_eq!(env.observe().node_index, 0);
    assert_eq!(env.observe().resources, before);
    assert!(env.legal_actions().is_empty());
    assert!(env.settlements().is_empty());
    assert!(env.transitions().is_empty());
    let replay = env
        .sample(&command, &mut || panic!("cancelled attempt cannot reroll"))
        .unwrap();
    assert_eq!(json(&replay), json(&step.attempt));
}

#[test]
fn action_limit_is_interrupted_even_when_a_defeat_rule_exists() {
    let mut input = spec();
    let battle = &mut input.nodes[0].battle;
    battle.waves[0][0].actions = battle.allies[0].actions.clone();
    battle.max_actions = 1;
    let mut env = RunEnv::new(input, "limited".into(), 17).unwrap();
    let step = env
        .step(&request(&env, "limited-fight"), &mut || false)
        .unwrap();
    assert_eq!(step.attempt.resolution.result.actions, 1);
    assert_eq!(step.attempt.resolution.result.outcome, Outcome::Incomplete);
    assert_eq!(
        step.attempt.resolution.diagnostic.incomplete_reason,
        Some(Termination::ActionLimit)
    );
    assert_eq!(env.observe().resources.hp, 30.0);
    assert_eq!(env.observe().resources.gold, 10);
    assert_eq!(env.observe().status, RunStatus::Interrupted);
    assert!(env.settlements().is_empty());
    assert!(env.transitions().is_empty());
}

#[test]
fn reaching_zero_run_hp_ends_the_run_and_records_the_actual_applied_damage() {
    let mut input = spec();
    input.initial.hp = 5.0;
    let mut env = RunEnv::new(input, "lethal-run-loss".into(), 17).unwrap();
    let step = env
        .step(&request(&env, "last-fight"), &mut || false)
        .unwrap();
    let receipt = step.settlement.unwrap();
    assert_eq!(receipt.requested_delta.hp, -7.0);
    assert_eq!(receipt.applied_delta.hp, -5.0);
    assert_eq!(env.observe().resources.hp, 0.0);
    assert_eq!(env.observe().resources.gold, 12);
    assert_eq!(env.observe().status, RunStatus::Defeated);
    assert!(env.legal_actions().is_empty());
    assert_eq!(env.attempts().len(), 1);
    assert_eq!(env.transitions().len(), 1);
    assert!(env.transitions()[0].terminated);
    assert!(!env.transitions()[0].truncated);
}

#[test]
fn zero_initial_hp_has_no_legal_battle_and_no_fabricated_failure_experience() {
    let mut input = spec();
    input.initial.hp = 0.0;
    let env = RunEnv::new(input, "already-ended".into(), 17).unwrap();
    assert_eq!(env.observe().status, RunStatus::Defeated);
    assert!(env.legal_actions().is_empty());
    assert!(env.attempts().is_empty());
    assert!(env.transitions().is_empty());
}

#[test]
fn nonfinite_hp_settlement_is_rejected_atomically() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let (mut env, attempt) = pending();
        let before = json(&env.report());
        let mut rules = loss_rules();
        rules.defeat.as_mut().unwrap().hp = value;
        let command = settlement_request(&env, &attempt, "invalid-hp", rules);
        let error = env.settle(&command).unwrap_err();
        assert!(error.contains("HP"), "{error}");
        assert_eq!(json(&env.report()), before);
    }
}

#[test]
fn currency_underflow_and_overflow_reject_the_entire_settlement() {
    for (gold, crystals, gold_delta, crystal_delta, expected_field) in [
        (10, 0, -11, 0, "gold"),
        (10, 0, 2, -1, "crystal"),
        (u32::MAX, 0, 1, 0, "gold"),
        (10, u32::MAX, 2, 1, "crystal"),
    ] {
        let mut input = spec();
        input.initial.gold = gold;
        input.initial.crystals = crystals;
        input.nodes[0].settlement = None;
        let mut env = RunEnv::new(input, "balance-check".into(), 17).unwrap();
        let attempt = env.sample(&request(&env, "sample"), &mut || false).unwrap();
        let before = json(&env.report());
        let mut rules = loss_rules();
        rules.defeat.as_mut().unwrap().gold = gold_delta;
        rules.defeat.as_mut().unwrap().crystals = crystal_delta;
        let command = settlement_request(&env, &attempt, "bad-balance", rules);
        let error = env.settle(&command).unwrap_err();
        assert!(error.contains(expected_field), "{error}");
        // HP -7 and earlier currency changes must not leak through the failure.
        assert_eq!(json(&env.report()), before);
        assert_eq!(env.observe().resources.hp, 30.0);
    }
}

#[test]
fn observation_contains_only_current_policy_visible_information() {
    let mut input = spec();
    input.nodes[1].id = "future-node-must-stay-hidden".into();
    input.nodes[1].battle.provenance.description = "future-battle-must-stay-hidden".into();
    let seed = u64::MAX;
    let env = RunEnv::new(input, "visible-run-id".into(), seed).unwrap();
    let observation = json(&env.observe());
    let mut keys: Vec<_> = observation
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "account_config_id",
            "current_node_id",
            "legal_actions",
            "node_index",
            "resources",
            "revision",
            "ruleset",
            "run_id",
            "status",
        ]
    );
    let encoded = observation.to_string();
    assert!(!encoded.contains("future-node-must-stay-hidden"));
    assert!(!encoded.contains("future-battle-must-stay-hidden"));
    assert!(!encoded.contains(&seed.to_string()));
    assert!(!encoded.contains("seed"));
    assert!(!encoded.contains("waves"));
}

#[test]
fn runs_and_cloned_environments_are_isolated_and_replay_the_same_seed() {
    let mut a = RunEnv::new(spec(), "run-a".into(), u64::MAX).unwrap();
    let mut b = RunEnv::new(spec(), "run-b".into(), u64::MAX).unwrap();
    let first_a = a.step(&request(&a, "first"), &mut || false).unwrap();
    let a_before_other_work = json(&a.report());
    let first_b = b.step(&request(&b, "first"), &mut || false).unwrap();
    assert_ne!(
        first_a.attempt.resolution.result.seed,
        first_b.attempt.resolution.result.seed
    );
    b.step(&request(&b, "second"), &mut || false).unwrap();
    assert_eq!(json(&a.report()), a_before_other_work);

    let mut fork = a.clone();
    fork.step(&request(&fork, "second"), &mut || false).unwrap();
    assert_eq!(json(&a.report()), a_before_other_work);
    a.step(&request(&a, "second"), &mut || false).unwrap();
    assert_eq!(json(&a.report()), json(&fork.report()));

    let mut replay = RunEnv::new(spec(), "run-a".into(), u64::MAX).unwrap();
    replay
        .step(&request(&replay, "first"), &mut || false)
        .unwrap();
    replay
        .step(&request(&replay, "second"), &mut || false)
        .unwrap();
    assert_eq!(json(&a.report()), json(&replay.report()));
    assert_eq!(json(&a.report())["master_seed"], u64::MAX.to_string());
    assert!(!a.report().seed_derivation.trim().is_empty());
    assert!(json(&a.report())["attempts"][0]["resolution"]["result"]["seed"].is_string());
}

#[test]
fn offline_batch_does_not_mutate_an_online_run_or_its_experience() {
    let mut env = env();
    env.step(&request(&env, "online-first"), &mut || false)
        .unwrap();
    let before = json(&env.report());
    let battle = battle_core::model::validate(spec().nodes[0].battle.clone()).unwrap();
    let batch = battle_core::batch::run_batch(
        &battle,
        &battle_core::batch::BatchRequest {
            trials: 3,
            seed: 17,
        },
        &mut || false,
    )
    .unwrap();
    assert_eq!(batch.losses, 3);
    assert_eq!(json(&env.report()), before);
    assert_eq!(env.transitions().len(), 1);
}

#[test]
fn final_victory_preserves_negative_av_margin_and_positive_overrun() {
    let mut input = spec();
    let mut node = input.nodes.remove(1);
    node.battle.max_av = 100.0;
    node.battle.max_actions = 1;
    node.battle.waves[0][0].revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 75.0,
    });
    match &mut node.battle.allies[0].actions[0].effects[0] {
        Effect::Damage { hits, .. } => *hits = 2,
        _ => panic!("synthetic lethal action is damage"),
    }
    input.nodes = vec![node];
    let mut env = RunEnv::new(input, "overrun-win".into(), 17).unwrap();
    let mut polls = 0;
    let step = env
        .step(&request(&env, "win"), &mut || {
            polls += 1;
            polls > 1
        })
        .unwrap();
    assert_eq!(
        polls, 1,
        "settled victory must not be overridden by later cancellation"
    );
    let result = &step.attempt.resolution;
    assert_eq!(result.result.outcome, Outcome::Victory);
    assert_eq!(result.result.termination, Termination::EnemiesDefeated);
    assert_eq!(result.result.elapsed_av, 50.0);
    assert_eq!(result.result.penalty_av, 75.0);
    assert_eq!(result.diagnostic.budget_used_av, 125.0);
    assert_eq!(result.diagnostic.budget_margin_av, -25.0);
    assert_eq!(result.diagnostic.remaining_av, 0.0);
    assert_eq!(result.diagnostic.overrun_av, 25.0);
    assert_eq!(result.diagnostic.success_remaining_av, Some(0.0));
    assert_eq!(result.diagnostic.failure_reason, None);
    assert_eq!(env.observe().status, RunStatus::Completed);
    assert_eq!(env.observe().resources.gold, 15);
}

#[test]
fn unknown_fields_and_unsupported_game_contracts_are_not_silently_defaulted() {
    let mut input = json(&spec());
    input["real_game_hp_formula"] = serde_json::json!(true);
    let error = serde_json::from_value::<RunSpec>(input).unwrap_err();
    assert!(error.to_string().contains("unknown field"));
    let mut input = spec();
    input.ruleset = "real_currency_wars".into();
    let error = RunEnv::new(input, "not-supported".into(), 17)
        .err()
        .expect("unsupported ruleset must be rejected");
    assert!(error.contains("ruleset"), "{error}");
}
