#![allow(dead_code)]
//! Synthetic scenarios execute the real battle engine; these are not game kits.
use run_core::*;
use serde::Serialize;
use serde_json::Value;

pub fn spec() -> RunSpec {
    serde_json::from_str(include_str!("../../examples/two-battles.json"))
        .expect("checked-in synthetic run schema")
}

pub fn env() -> RunEnv {
    RunEnv::new(spec(), "test-run".into(), 17).expect("valid synthetic run")
}

pub fn request(env: &RunEnv, id: &str) -> SampleRequest {
    let observation = env.observe();
    SampleRequest {
        request_id: id.into(),
        node_id: observation.current_node_id.expect("current battle node"),
        expected_revision: observation.revision,
        trace: true,
    }
}

pub fn settlement_request(
    env: &RunEnv,
    attempt: &BattleAttempt,
    id: &str,
    rules: SettlementRules,
) -> SettlementRequest {
    SettlementRequest {
        request_id: id.into(),
        node_id: attempt.node_id.clone(),
        attempt_id: attempt.attempt_id.clone(),
        expected_revision: env.observe().revision,
        rules,
    }
}

pub fn loss_rules() -> SettlementRules {
    spec().nodes[0].settlement.clone().unwrap()
}

pub fn json(value: &impl Serialize) -> Value {
    serde_json::to_value(value).expect("finite serializable result")
}

pub fn pending() -> (RunEnv, BattleAttempt) {
    let mut input = spec();
    input.nodes[0].settlement = None;
    let mut env = RunEnv::new(input, "pending-run".into(), 17).unwrap();
    let command = request(&env, "sample-loss");
    let result = env.step(&command, &mut || false).unwrap();
    assert!(result.settlement.is_none());
    assert_eq!(env.observe().status, RunStatus::AwaitingSettlement);
    (env, result.attempt)
}
