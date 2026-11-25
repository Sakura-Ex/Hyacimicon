//! Replay one ordinary timed node and preview its squad HP settlement.

use std::{env, fs};

use battle_core::diagnostics::{
    ScenarioFile,
    replay::{ReplayScript, replay},
};
use run_core::{
    HpChange, OrdinaryTimedNodeHpRule, SquadHpLosses, TimedNodeHpInput, TimedNodeHpSettlement,
    settle_ordinary_timed_node_hp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Caller-resolved context for an ordinary timed node without a threshold penalty.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeContext {
    hp_before: u32,
    hp_cap: u32,
    base_loss: u32,
    progress_loss_coefficient: u32,
    progress_percent: Option<u8>,
    post_battle_healing: Option<u32>,
}

impl NodeContext {
    fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        if bytes
            .iter()
            .copied()
            .find(|byte| !byte.is_ascii_whitespace())
            != Some(b'{')
        {
            return Err("expected a JSON object".into());
        }
        serde_json::from_slice(bytes).map_err(|error| error.to_string())
    }

    fn settlement_input(&self) -> TimedNodeHpInput {
        TimedNodeHpInput {
            hp_before: self.hp_before,
            hp_cap: self.hp_cap,
            rule: OrdinaryTimedNodeHpRule {
                base_loss: self.base_loss,
                progress_loss_coefficient: self.progress_loss_coefficient,
            },
            progress_percent: self.progress_percent,
            post_battle_healing: self.post_battle_healing,
        }
    }
}

fn change_report(change: HpChange) -> Value {
    json!({"requested": change.requested, "applied": change.applied})
}

fn loss_report(losses: SquadHpLosses) -> Value {
    json!({
        "base": change_report(losses.base),
        "progress": change_report(losses.progress),
        "hp_after_losses": losses.hp_after_losses,
    })
}

/// Encode only values established by the preview.
fn settlement_report(settlement: TimedNodeHpSettlement) -> Value {
    match settlement {
        TimedNodeHpSettlement::Incomplete => json!({"status": "incomplete"}),
        TimedNodeHpSettlement::ProgressUnknown => json!({"status": "progress_unknown"}),
        TimedNodeHpSettlement::RecoveryUnknown { losses } => {
            json!({"status": "recovery_unknown", "losses": loss_report(losses)})
        }
        TimedNodeHpSettlement::Settled {
            losses,
            recovery,
            hp_after,
        } => json!({
            "status": "settled",
            "losses": loss_report(losses),
            "recovery": recovery.map(change_report),
            "hp_after": hp_after,
        }),
    }
}

fn preview(
    scenario: &ScenarioFile,
    seed: u64,
    operations: &ReplayScript,
    node: NodeContext,
) -> Result<Value, String> {
    if scenario.settlement.is_some() {
        return Err(
            "scenario: remove mechanics.settlement to preview the ordinary timed node".into(),
        );
    }
    let battle = replay(scenario, seed, operations).map_err(|error| format!("replay: {error}"))?;
    let squad_hp = settle_ordinary_timed_node_hp(battle.battle_result(), node.settlement_input())
        .map_err(|error| format!("squad HP: {error}"))?;
    Ok(json!({
        "battle": battle,
        "node_context": node,
        "ordinary_timed_node_hp": settlement_report(squad_hp),
    }))
}

fn main() -> Result<(), String> {
    let args = env::args().collect::<Vec<_>>();
    let [_, scenario_path, seed, operations_path, context_path] = args.as_slice() else {
        return Err("usage: replay_timed_node SCENARIO.json SEED OPERATIONS.json NODE.json".into());
    };
    let seed = seed
        .parse()
        .map_err(|_| "seed must be an unsigned integer")?;
    let read = |path: &str| fs::read(path).map_err(|error| format!("{path}: {error}"));
    let scenario = ScenarioFile::from_slice(&read(scenario_path)?)
        .map_err(|error| format!("scenario: {error}"))?;
    let operations = serde_json::from_slice(&read(operations_path)?)
        .map_err(|error| format!("operations: {error}"))?;
    let node = NodeContext::from_slice(&read(context_path)?)
        .map_err(|error| format!("node context: {error}"))?;
    let report = preview(&scenario, seed, &operations, node)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_itemized_hp_after_the_existing_revival_replay() {
        let scenario =
            ScenarioFile::from_slice(include_bytes!("../../battle/examples/revival-budget.json"))
                .unwrap();
        let script = serde_json::from_slice(include_bytes!(
            "../../battle/examples/revival-budget-operations.json"
        ))
        .unwrap();
        let node = NodeContext::from_slice(
            br#"{"hp_before":92,"hp_cap":100,"base_loss":5,"progress_loss_coefficient":10,
                 "progress_percent":20,"post_battle_healing":2}"#,
        )
        .unwrap();

        let report = preview(&scenario, 42, &script, node).unwrap();
        assert_eq!(report["battle"]["outcome"], json!("defeat"));
        assert_eq!(
            report["battle"]["termination"],
            json!("av_budget_exhausted")
        );
        assert_eq!(report["battle"]["remaining_av"], json!(0.0));
        assert_eq!(report["battle"]["penalty_av"], json!(50.0));
        assert_eq!(
            report["ordinary_timed_node_hp"],
            json!({
                "status": "settled",
                "losses": {
                    "base": {"requested": 5, "applied": 5},
                    "progress": {"requested": 8, "applied": 8},
                    "hp_after_losses": 79,
                },
                "recovery": {"requested": 2, "applied": 2},
                "hp_after": 81,
            })
        );
    }
}
