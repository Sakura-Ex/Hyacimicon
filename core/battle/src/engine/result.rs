//! 将战斗状态导出为结果并执行声明的临时 HP 结算。
use super::*;

impl State<'_> {
    pub(super) fn finish(
        self,
        seed: u64,
        outcome: Outcome,
        termination: Termination,
        waves_cleared: usize,
    ) -> BattleResult {
        let weaknesses: Vec<_> = (0..self.units.len())
            .map(|i| self.weakness_state(i))
            .collect();
        let settlement = self
            .input
            .scenario()
            .mechanics
            .settlement
            .as_ref()
            .map(|p| {
                let defeated = self
                    .units
                    .iter()
                    .filter(|u| !u.ally && !u.alive())
                    .map(|u| u.spec.id.clone())
                    .collect();
                crate::settlement::settle(p, defeated, outcome)
            });
        let run_hp_loss = settlement.as_ref().and_then(|s| s.actual_loss);
        let queue = self
            .input
            .scenario()
            .mechanics
            .queue_capacity
            .map(|capacity| QueueResult {
                policy: "finite_queue_after_action_v1".into(),
                capacity,
                active_slots: self
                    .queue_slots
                    .iter()
                    .map(|i| {
                        i.filter(|&i| self.units[i].alive())
                            .map(|i| self.units[i].spec.id.clone())
                    })
                    .collect(),
                remaining_ids: self.input.scenario().waves[0][self.queue_next..]
                    .iter()
                    .map(|u| u.id.clone())
                    .collect(),
            });
        BattleResult {
            queue,
            settlement,
            assumptions: self.input.scenario().mechanics.assumptions.clone(),
            provenance: self.input.scenario().provenance.clone(),
            scenario_sha256: self.input.fingerprint().into(),
            seed,
            ruleset: "battle_v2".into(),
            cycle: self.timeline.cycle + 1,
            cycle_remaining_av: self.timeline.remaining,
            remaining_av: self.timeline.remaining_total(),
            outcome,
            termination,
            elapsed_av: self.timeline.now(),
            penalty_av: self.timeline.penalty(),
            actions: self.actions,
            waves_cleared,
            ally_skill_points: self.sp[0],
            enemy_skill_points: self.sp[1],
            units: self
                .units
                .into_iter()
                .zip(weaknesses)
                .map(|(u, weaknesses)| u.snapshot(weaknesses))
                .collect(),
            events: self.events,
            trace_truncated: self.trace_truncated,
            run_hp_loss,
            validation_scope:
                "explicit_battle_v2_operations; imported_config_coverage_is_recorded_separately"
                    .into(),
        }
    }
}
