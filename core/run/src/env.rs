use crate::{model::*, resolution::resolve_once, settlement};
use battle_core::model::{validate, Outcome, ValidatedScenario};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
struct PreparedNode {
    id: String,
    battle: ValidatedScenario,
    settlement: Option<SettlementRules>,
}

/// Single-owner environment. Cloning creates an isolated candidate copy; the
/// original cannot accept a replacement BattleResult from that candidate.
#[derive(Clone)]
pub struct RunEnv {
    run_id: String,
    account_config_id: String,
    master_seed: u64,
    spec_sha256: String,
    ruleset: String,
    provenance: battle_core::model::Provenance,
    nodes: Vec<PreparedNode>,
    revision: u32,
    node_index: u32,
    resources: Resources,
    status: RunStatus,
    pending: Option<usize>,
    attempts: Vec<BattleAttempt>,
    settlements: Vec<SettlementReceipt>,
    transitions: Vec<TrainingTransition>,
    sample_commands: BTreeMap<String, (SampleRequest, usize)>,
    settlement_commands: BTreeMap<String, (Vec<u8>, usize)>,
}

fn encoded(value: &impl Serialize) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}
fn fingerprint(value: &impl Serialize) -> Result<String, String> {
    Ok(format!("{:x}", Sha256::digest(encoded(value)?)))
}

/// Per-run and per-node streams, independent of batch sampling or worker order.
pub fn derive_node_seed(
    master_seed: u64,
    run_id: &str,
    node_index: u32,
    attempt_index: u32,
) -> u64 {
    let mut hash = Sha256::new();
    hash.update(b"hcwa/run/node-seed/v1\0");
    hash.update(master_seed.to_le_bytes());
    hash.update((run_id.len() as u64).to_le_bytes());
    hash.update(run_id.as_bytes());
    hash.update(node_index.to_le_bytes());
    hash.update(attempt_index.to_le_bytes());
    let digest = hash.finalize();
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

impl RunEnv {
    pub fn new(spec: RunSpec, run_id: String, seed: u64) -> Result<Self, String> {
        settlement::identifier(&run_id, "run_id")?;
        settlement::identifier(&spec.account_config_id, "account_config_id")?;
        if spec.schema_version != 1 || spec.ruleset != "synthetic_run_v1" {
            return Err(
                "unsupported run schema/ruleset; only synthetic_run_v1 is implemented".into(),
            );
        }
        for (text, name) in [
            (&spec.provenance.game_version, "game_version"),
            (&spec.provenance.data_snapshot, "data_snapshot"),
            (&spec.provenance.description, "description"),
        ] {
            settlement::identifier(text, name)?;
        }
        if spec.nodes.is_empty() || spec.nodes.len() > 10_000 {
            return Err("run must contain 1..10000 explicit nodes".into());
        }
        settlement::validate_resources(&spec.initial)?;
        let spec_sha256 = fingerprint(&spec)?;
        let mut ids = BTreeSet::new();
        let mut nodes = Vec::new();
        for node in spec.nodes {
            settlement::identifier(&node.id, "node id")?;
            if !ids.insert(node.id.clone()) {
                return Err("duplicate node id".into());
            }
            if let Some(rules) = &node.settlement {
                settlement::validate_rules(rules)?;
            }
            nodes.push(PreparedNode {
                id: node.id,
                battle: validate(node.battle)?,
                settlement: node.settlement,
            });
        }
        let status = if spec.initial.hp == 0.0 {
            RunStatus::Defeated
        } else {
            RunStatus::Ready
        };
        Ok(Self {
            run_id,
            account_config_id: spec.account_config_id,
            master_seed: seed,
            spec_sha256,
            ruleset: spec.ruleset,
            provenance: spec.provenance,
            nodes,
            revision: 0,
            node_index: 0,
            resources: spec.initial,
            status,
            pending: None,
            attempts: Vec::new(),
            settlements: Vec::new(),
            transitions: Vec::new(),
            sample_commands: BTreeMap::new(),
            settlement_commands: BTreeMap::new(),
        })
    }

    pub fn legal_actions(&self) -> Vec<RunAction> {
        if self.status == RunStatus::Ready {
            vec![RunAction::FightCurrentNode]
        } else {
            vec![]
        }
    }

    pub fn observe(&self) -> RunObservation {
        let terminal = matches!(self.status, RunStatus::Completed | RunStatus::Defeated);
        RunObservation {
            run_id: self.run_id.clone(),
            account_config_id: self.account_config_id.clone(),
            revision: self.revision,
            node_index: self.node_index,
            current_node_id: if terminal {
                None
            } else {
                self.nodes
                    .get(self.node_index as usize)
                    .map(|n| n.id.clone())
            },
            resources: self.resources.clone(),
            status: self.status,
            ruleset: self.ruleset.clone(),
            legal_actions: self.legal_actions(),
        }
    }

    pub fn attempts(&self) -> &[BattleAttempt] {
        &self.attempts
    }
    pub fn settlements(&self) -> &[SettlementReceipt] {
        &self.settlements
    }
    pub fn transitions(&self) -> &[TrainingTransition] {
        &self.transitions
    }

    /// A successful request ID is immutable. Exact retries return its original
    /// record without running the engine or polling the cancellation callback.
    pub fn sample(
        &mut self,
        request: &SampleRequest,
        cancel: &mut impl FnMut() -> bool,
    ) -> Result<BattleAttempt, String> {
        settlement::identifier(&request.request_id, "sample request_id")?;
        if let Some((prior, index)) = self.sample_commands.get(&request.request_id) {
            return if prior == request {
                Ok(self.attempts[*index].clone())
            } else {
                Err("sample request_id conflict".into())
            };
        }
        if self.settlement_commands.contains_key(&request.request_id) {
            return Err("request_id already used for settlement".into());
        }
        if request.expected_revision != self.revision {
            return Err("stale sample revision".into());
        }
        if self.status != RunStatus::Ready {
            return Err("run is not ready to sample; no automatic retry".into());
        }
        let node = &self.nodes[self.node_index as usize];
        if node.id != request.node_id {
            return Err("sample node mismatch".into());
        }
        let next_revision = self.revision.checked_add(1).ok_or("revision overflow")?;
        let attempt_index =
            u32::try_from(self.attempts.len()).map_err(|_| "attempt index overflow")?;
        let seed = derive_node_seed(
            self.master_seed,
            &self.run_id,
            self.node_index,
            attempt_index,
        );
        let before = self.observe();
        let resolution = resolve_once(&node.battle, seed, request.trace, cancel)?;
        let incomplete = resolution.result.outcome == Outcome::Incomplete;
        let attempt = BattleAttempt {
            attempt_id: format!(
                "{}:node-{}:attempt-{}",
                self.run_id, self.node_index, attempt_index
            ),
            request_id: request.request_id.clone(),
            node_id: node.id.clone(),
            before,
            sampled_revision: next_revision,
            resolution,
            provenance: self.provenance.clone(),
            battle_provenance: node.battle.scenario().provenance.clone(),
            origin: "simulated_episode_with_explicit_synthetic_settlement".into(),
        };
        let index = self.attempts.len();
        self.attempts.push(attempt.clone());
        self.sample_commands
            .insert(request.request_id.clone(), (request.clone(), index));
        self.revision = next_revision;
        self.status = if incomplete {
            RunStatus::Interrupted
        } else {
            RunStatus::AwaitingSettlement
        };
        self.pending = if incomplete { None } else { Some(index) };
        Ok(attempt)
    }

    /// Apply only the stored outcome. All checks and arithmetic complete before
    /// resources, position, revision, receipts, or transitions are changed.
    pub fn settle(&mut self, request: &SettlementRequest) -> Result<SettlementReceipt, String> {
        settlement::identifier(&request.request_id, "settlement request_id")?;
        settlement::validate_rules(&request.rules)?;
        let request_bytes = encoded(request)?;
        if let Some((prior, index)) = self.settlement_commands.get(&request.request_id) {
            return if *prior == request_bytes {
                Ok(self.settlements[*index].clone())
            } else {
                Err("settlement request_id conflict".into())
            };
        }
        if self.sample_commands.contains_key(&request.request_id) {
            return Err("request_id already used for sampling".into());
        }
        if self
            .settlements
            .iter()
            .any(|r| r.attempt_id == request.attempt_id)
        {
            return Err("attempt already settled; receipt cannot be rewritten".into());
        }
        if request.expected_revision != self.revision {
            return Err("stale settlement revision".into());
        }
        if self.status != RunStatus::AwaitingSettlement {
            return Err("run has no completed pending battle".into());
        }
        let index = self.pending.ok_or("pending attempt missing")?;
        let attempt = &self.attempts[index];
        if attempt.attempt_id != request.attempt_id
            || attempt.node_id != request.node_id
            || self.nodes[self.node_index as usize].id != request.node_id
        {
            return Err("settlement attempt/node mismatch".into());
        }
        let outcome = attempt.resolution.result.outcome;
        let delta = settlement::applicable(&request.rules, outcome)
            .ok_or("settlement rule missing for actual battle outcome")?
            .clone();
        let (resources, applied_delta) = settlement::apply(&self.resources, &delta)?;
        let revision = self.revision.checked_add(1).ok_or("revision overflow")?;
        let node_index = self
            .node_index
            .checked_add(1)
            .ok_or("node index overflow")?;
        let status = if resources.hp == 0.0 {
            RunStatus::Defeated
        } else if node_index as usize == self.nodes.len() {
            RunStatus::Completed
        } else {
            RunStatus::Ready
        };
        let after = RunObservation {
            run_id: self.run_id.clone(),
            account_config_id: self.account_config_id.clone(),
            revision,
            node_index,
            current_node_id: if status == RunStatus::Ready {
                Some(self.nodes[node_index as usize].id.clone())
            } else {
                None
            },
            resources: resources.clone(),
            status,
            ruleset: self.ruleset.clone(),
            legal_actions: if status == RunStatus::Ready {
                vec![RunAction::FightCurrentNode]
            } else {
                vec![]
            },
        };
        let receipt = SettlementReceipt {
            request_id: request.request_id.clone(),
            attempt_id: request.attempt_id.clone(),
            node_id: request.node_id.clone(),
            expected_revision: request.expected_revision,
            outcome,
            rules: request.rules.clone(),
            rules_sha256: fingerprint(&request.rules)?,
            requested_delta: delta,
            applied_delta,
            before: self.observe(),
            after: after.clone(),
        };
        let transition = TrainingTransition {
            before: attempt.before.clone(),
            action: RunAction::FightCurrentNode,
            attempt: attempt.clone(),
            settlement: receipt.clone(),
            after,
            terminated: matches!(status, RunStatus::Completed | RunStatus::Defeated),
            truncated: false,
            reward: None,
        };
        let receipt_index = self.settlements.len();
        self.resources = resources;
        self.revision = revision;
        self.node_index = node_index;
        self.status = status;
        self.pending = None;
        self.settlements.push(receipt.clone());
        self.transitions.push(transition);
        self.settlement_commands
            .insert(request.request_id.clone(), (request_bytes, receipt_index));
        Ok(receipt)
    }

    /// Normal run path: one sample, then settlement if the actual outcome has an
    /// explicit rule. Unknown rules stop here; a later settle never resamples.
    pub fn step(
        &mut self,
        request: &SampleRequest,
        cancel: &mut impl FnMut() -> bool,
    ) -> Result<StepResult, String> {
        let attempt = self.sample(request, cancel)?;
        if let Some(receipt) = self
            .settlements
            .iter()
            .find(|r| r.attempt_id == attempt.attempt_id)
        {
            return Ok(StepResult {
                attempt,
                settlement: Some(receipt.clone()),
                observation: receipt.after.clone(),
                pending_reason: None,
            });
        }
        if attempt.resolution.result.outcome == Outcome::Incomplete {
            return Ok(StepResult {
                attempt,
                settlement: None,
                observation: self.observe(),
                pending_reason: Some("battle incomplete; no settlement or loss transition".into()),
            });
        }
        let node = self
            .nodes
            .iter()
            .find(|n| n.id == attempt.node_id)
            .ok_or("attempt node missing")?;
        let rules = match &node.settlement {
            Some(rules)
                if settlement::applicable(rules, attempt.resolution.result.outcome).is_some() =>
            {
                rules.clone()
            }
            _ => {
                return Ok(StepResult {
                    attempt,
                    settlement: None,
                    observation: self.observe(),
                    pending_reason: Some("settlement rule unknown for actual outcome".into()),
                })
            }
        };
        let auto_id = format!(
            "auto-settle-{:x}",
            Sha256::digest(request.request_id.as_bytes())
        );
        let receipt = self.settle(&SettlementRequest {
            request_id: auto_id,
            node_id: attempt.node_id.clone(),
            attempt_id: attempt.attempt_id.clone(),
            expected_revision: attempt.sampled_revision,
            rules,
        })?;
        Ok(StepResult {
            attempt,
            observation: receipt.after.clone(),
            settlement: Some(receipt),
            pending_reason: None,
        })
    }

    /// Audit/training export; this is deliberately separate from policy observation.
    pub fn report(&self) -> RunReport {
        RunReport {
            spec_sha256: self.spec_sha256.clone(),
            master_seed: self.master_seed,
            seed_derivation: "SHA256 domain hcwa/run/node-seed/v1\\0 || master_u64_le || run_id_utf8_byte_length_u64_le || run_id_utf8 || node_index_u32_le || attempt_index_u32_le; first 8 digest bytes as u64_le".into(),
            observation: self.observe(), attempts: self.attempts.clone(),
            settlements: self.settlements.clone(), transitions: self.transitions.clone(),
            scope: "synthetic run foundation; one battle sample per node, explicit settlement, no live-game formula or trained policy".into(),
        }
    }
}
