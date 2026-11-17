//! 合法操作、策略选择与动作入口的资源结算。
use super::*;

impl State<'_> {
    pub(super) fn pay(&mut self, actor: usize, action: &Action) {
        let side = self.side(actor);
        self.sp[side] -= action.sp_cost;
        self.units[actor].energy -= action.energy_cost;
        self.emit(
            "energy_cost",
            actor,
            Some(actor),
            Some(&action.id),
            Some(action.energy_cost),
            None,
        );
    }

    pub(super) fn enqueue(&mut self, request: Pending) -> Result<(), String> {
        if let Some(a) = request.action {
            let kind = self.units[request.actor].spec.actions[a].kind;
            if (request.kind == RequestKind::Ultimate && kind != ActionKind::Ultimate)
                || (request.kind == RequestKind::FollowUp && kind != ActionKind::FollowUp)
            {
                return Err("request/action kind mismatch".into());
            }
        }
        if request.kind == RequestKind::Ultimate
            && self
                .pending
                .iter()
                .any(|p| p.actor == request.actor && p.kind == RequestKind::Ultimate)
        {
            return Ok(());
        }
        if self.pending.len() >= 10000 {
            return Err("pending action limit exceeded".into());
        }
        self.emit(
            "request_queued",
            request.actor,
            None,
            None,
            Some(self.units[request.actor].energy),
            None,
        );
        self.pending.push_back(request);
        Ok(())
    }

    pub(super) fn decide(
        &self,
        point: DecisionPoint,
        actor: Option<usize>,
        policy: &mut impl Policy,
    ) -> Result<Option<Operation>, String> {
        let mut legal = Vec::new();
        for (i, u) in self.units.iter().enumerate() {
            if !u.alive() || actor.is_some_and(|a| a != i) {
                continue;
            }
            if point == DecisionPoint::Insert
                && self
                    .pending
                    .iter()
                    .any(|p| p.actor == i && p.kind == RequestKind::Ultimate)
            {
                continue;
            }
            for (j, a) in u.spec.actions.iter().enumerate() {
                let correct = if point == DecisionPoint::Insert {
                    a.kind == ActionKind::Ultimate
                } else {
                    matches!(a.kind, ActionKind::Basic | ActionKind::Skill)
                };
                if correct && self.affordable(i, a) {
                    legal.push(Operation {
                        actor: i,
                        action: j,
                    });
                }
            }
        }
        if point == DecisionPoint::Insert {
            let rules = &self.input.scenario().mechanics;
            legal.sort_by(|a, b| {
                let priority = |op: &Operation| {
                    *rules
                        .ultimate_priority
                        .get(&self.units[op.actor].spec.id)
                        .unwrap_or(&0)
                };
                let order = priority(a).cmp(&priority(b));
                (if rules.larger_priority_first {
                    order.reverse()
                } else {
                    order
                })
                .then(a.actor.cmp(&b.actor))
                .then(a.action.cmp(&b.action))
            });
        }
        let mut axis: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.alive())
            .map(|u| AxisPosition {
                id: u.spec.id.clone(),
                remaining_distance: u.remaining,
                speed: u.spec.stats.speed,
                waiting_av: u.remaining / u.spec.stats.speed,
            })
            .collect();
        axis.sort_by(|a, b| a.waiting_av.total_cmp(&b.waiting_av));
        let observation = Observation {
            now: self.timeline.now,
            cycle: self.timeline.cycle + 1,
            cycle_remaining_av: self.timeline.remaining,
            axis,
            units: self
                .units
                .iter()
                .enumerate()
                .map(|(owner, u)| u.snapshot(self.weakness_state(owner)))
                .collect(),
        };
        let chosen = policy.decide(point, &observation, &legal);
        if chosen.as_ref().is_some_and(|op| !legal.contains(op)) {
            return Err("policy returned an illegal operation".into());
        }
        Ok(chosen)
    }

    pub(super) fn act(
        &mut self,
        actor: usize,
        action_index: usize,
        prepaid: bool,
    ) -> Result<(), String> {
        let action = self.units[actor].spec.actions[action_index].clone();
        if !prepaid && !self.affordable(actor, &action) {
            return Err("selected action is not affordable".into());
        }
        // Bind once even when a nested conditional/loop is the first target use.
        self.bound_target = self.targets(actor, Target::EnemyLowestHp)?.first().copied();
        self.current_action_kind = Some(action.kind);
        self.attack_owner = Some(actor);
        self.attack_targets.clear();
        let side = self.side(actor);
        if !prepaid {
            self.pay(actor, &action);
        }
        self.actions += 1;
        self.dispatch(BattleEvent::ActionStart, Some(actor), Some(&action.id))?;
        self.dispatch(BattleEvent::BeforeSkillUse, Some(actor), Some(&action.id))?;
        self.emit("action", actor, None, Some(&action.id), None, None);
        self.apply_effects(actor, &action.id, &action.effects)?;
        if self.units[actor].alive() {
            self.sp[side] = self.sp[side]
                .saturating_add(action.sp_gain)
                .min(self.sp_max(side));
            self.units[actor].energy = (self.units[actor].energy + action.energy_gain)
                .min(self.units[actor].spec.max_energy);
        }
        self.dispatch(BattleEvent::ActionEnd, Some(actor), Some(&action.id))?;
        self.emit("action_end", actor, None, Some(&action.id), None, None);
        self.bound_target = None;
        self.current_action_kind = None;
        self.attack_owner = None;
        self.attack_targets.clear();
        Ok(())
    }
}
