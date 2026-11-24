//! 可暂停的战斗推进：引擎产生决策，调用方选招，再单独提交执行。
use super::*;

/// 推进到的外部可见边界。借用结束后可以向同一会话提交选择。
#[derive(Debug)]
pub enum BattleStep<'session> {
    /// 等待调用方回答的决策引用。
    Decision(&'session Decision),
    /// 已经完成的战斗结果。
    Finished(&'session BattleResult),
}

/// 本场推进阶段；决策、完成结果与失败互斥。
enum Phase {
    /// 公共调度边界。
    Boundary,
    /// 插入选择提交后恢复排队请求或普通行动。
    AfterInsert,
    /// 等待外部策略提交的决策。
    Awaiting(Decision),
    /// 已经完成的战斗结果。
    Finished(Box<BattleResult>),
    /// 执行失败的原始原因，后续调用继续返回错误。
    Failed(String),
}

/// 保存随机流与运行状态的单场会话，供推理和训练逐步驱动。
///
/// `advance`推进到下一个决策或结果；`submit`验证并执行选择。
/// 等待外部推理期间，读取同一决策保持AV、资源和随机流不变。
pub struct BattleSession<'input> {
    /// 所属对象的当前运行状态。
    state: State<'input>,
    /// 下一个会话内递增决策编号。
    next_decision_id: u64,
    /// 当前互斥推进阶段。
    phase: Phase,
}

impl<'input> BattleSession<'input> {
    /// 创建独立状态，执行开战与首波事件，暂不调用任何选招策略。
    pub fn new(input: &'input Scenario, seed: u64) -> Result<Self, String> {
        Self::create(
            input,
            seed,
            #[cfg(feature = "diagnostics")]
            false,
            #[cfg(feature = "diagnostics")]
            None,
        )
    }

    /// 创建开发会话；trace 指定是否收集回放，规则执行与正式入口共用。
    #[cfg(feature = "diagnostics")]
    pub fn with_trace(input: &'input Scenario, seed: u64, trace: bool) -> Result<Self, String> {
        Self::create(input, seed, trace, None)
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn with_recorded_random(
        input: &'input Scenario,
        seed: u64,
        draws: Option<Vec<RecordedDraw>>,
    ) -> Result<Self, String> {
        Self::create(input, seed, true, draws)
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn set_random_step(&mut self, step: usize) {
        if let Some(recorded) = &mut self.state.recorded_random {
            recorded.set_step(step);
        }
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn check_random_interval(&self, terminal: bool) -> Result<(), String> {
        self.state
            .recorded_random
            .as_ref()
            .map_or(Ok(()), |recorded| recorded.check_remaining(terminal))
    }

    /// 从当前合法操作读取单位和动作标识，供诊断脚本比对。
    #[cfg(feature = "diagnostics")]
    pub(crate) fn operation_names(&self, op: &Operation) -> (&str, &str) {
        let unit = &self.state.units[self.state.runtime_actor_index(op.actor)].spec;
        (&unit.id, &unit.actions[op.action].id)
    }

    /// 读取当前波次编号，首波为 1。
    #[cfg(feature = "diagnostics")]
    pub(crate) fn replay_wave(&self) -> usize {
        self.state.wave + 1
    }

    /// Read the finished snapshot without advancing or copying the event trace.
    #[cfg(feature = "diagnostics")]
    pub(crate) fn replay_result(&self) -> Option<&BattleResult> {
        match &self.phase {
            Phase::Finished(result) => Some(result),
            _ => None,
        }
    }

    /// 读取已记录的诊断事件。终局时事件已经移入结果。
    #[cfg(feature = "diagnostics")]
    pub(crate) fn replay_trace(&self) -> Result<&[crate::diagnostics::Event], String> {
        let (events, truncated) = match &self.phase {
            Phase::Finished(result) => (&result.events, result.trace_truncated),
            _ => (&self.state.events, self.state.trace_truncated),
        };
        if truncated {
            return Err("battle trace was truncated".into());
        }
        Ok(events)
    }

    /// 初始化本场状态和开战事件，诊断选项在正式构建中被排除。
    fn create(
        input: &'input Scenario,
        seed: u64,
        #[cfg(feature = "diagnostics")] trace: bool,
        #[cfg(feature = "diagnostics")] draws: Option<Vec<RecordedDraw>>,
    ) -> Result<Self, String> {
        let mut state = State::new(
            input,
            seed,
            #[cfg(feature = "diagnostics")]
            trace,
        );
        #[cfg(feature = "diagnostics")]
        {
            state.recorded_random = draws.map(super::recorded_rng::RecordedRandom::new);
        }
        state.spawn_opening_summons()?;
        state.dispatch(BattleEvent::BattleStart, None, None)?;
        state.dispatch(BattleEvent::WaveStart, None, None)?;
        state.queue_opening_battle_event()?;
        Ok(Self {
            state,
            next_decision_id: 0,
            phase: Phase::Boundary,
        })
    }

    /// 推进到决策或结果。等待中的决策重复读取时保持状态不变。
    pub fn advance(&mut self) -> Result<BattleStep<'_>, String> {
        self.advance_cancellable(&mut || false)
    }

    /// 在动作边界检查取消；等待决策及提交插入后恢复推进时也检查取消。
    ///
    /// 已完成的结果优先返回。取消产生`Incomplete/Cancelled`快照，
    /// 完整Action执行期间保持原子性。运行错误结束会话并保留错误原因。
    pub fn advance_cancellable(
        &mut self,
        cancel: &mut impl FnMut() -> bool,
    ) -> Result<BattleStep<'_>, String> {
        // 已放弃插入且仅剩到期回合收尾时，先完成TurnEnd/CycleEnd。
        // 其他恢复点仍在推进时间或执行新Action前观察暂停期间的取消。
        let finishing_turn = matches!(self.phase, Phase::AfterInsert)
            && self.state.timeline.remaining == 0.0
            && self.state.turn.is_some()
            && self.state.pending.is_empty();
        if !finishing_turn
            && matches!(self.phase, Phase::Awaiting(_) | Phase::AfterInsert)
            && cancel()
        {
            self.conclude(Outcome::Incomplete, Termination::Cancelled);
        }
        self.advance_continuous(cancel)
    }

    /// 同步适配器连续选招后恢复原调度，沿用连续运行的边界取消轮询。
    /// 公开分步入口在调用本函数前另检查外部暂停期间的取消信号。
    pub(super) fn advance_continuous(
        &mut self,
        cancel: &mut impl FnMut() -> bool,
    ) -> Result<BattleStep<'_>, String> {
        if let Err(error) = self.advance_inner(cancel) {
            self.phase = Phase::Failed(error.clone());
            return Err(error);
        }
        match &self.phase {
            Phase::Awaiting(decision) => Ok(BattleStep::Decision(decision)),
            Phase::Finished(result) => Ok(BattleStep::Finished(result)),
            Phase::Failed(error) => Err(error.clone()),
            Phase::Boundary | Phase::AfterInsert => {
                unreachable!("advance_inner returns only at a decision or result")
            }
        }
    }

    /// 提交当前决策的选择。非法、过期和重复提交在修改状态前返回错误。
    ///
    /// 普通/额外Turn的选择执行一个完整Action；插入选择登记终结技请求，
    /// 下一次推进在其实际入Turn时支付能量。`None`仅用于放弃插入机会。
    /// 执行中的错误使会话进入失败状态，调用方应重新建立会话。
    pub fn submit(&mut self, decision_id: u64, choice: Option<Operation>) -> Result<(), String> {
        self.submit_choice(decision_id, choice, None)
    }

    /// List unambiguous living enemy IDs for a legal action that binds a primary target.
    pub fn legal_primary_targets(
        &self,
        decision_id: u64,
        choice: &Operation,
    ) -> Result<Vec<&str>, String> {
        let decision = self.awaiting_decision(decision_id)?;
        if !decision.legal.contains(choice) {
            return Err("policy returned an illegal operation".into());
        }
        let actor = self.state.runtime_actor_index(choice.actor);
        let mut names = std::collections::BTreeMap::new();
        for (_, name) in self.state.primary_enemy_candidates(actor, choice.action)? {
            names
                .entry(name)
                .and_modify(|unique| *unique = false)
                .or_insert(true);
        }
        Ok(names
            .into_iter()
            .filter_map(|(name, unique)| unique.then_some(name))
            .collect())
    }

    /// Submit a legal operation with a primary enemy identified by its unit ID.
    /// Validation leaves the current decision intact on error. An inserted
    /// ultimate retains the selected instance until it reaches the queue head.
    pub fn submit_targeted(
        &mut self,
        decision_id: u64,
        choice: Operation,
        target: &str,
    ) -> Result<(), String> {
        self.submit_choice(decision_id, Some(choice), Some(target))
    }

    fn awaiting_decision(&self, decision_id: u64) -> Result<&Decision, String> {
        let Phase::Awaiting(decision) = &self.phase else {
            return Err("battle is not awaiting a decision".into());
        };
        if decision.id != decision_id {
            return Err("stale decision id".into());
        }
        Ok(decision)
    }

    fn submit_choice(
        &mut self,
        decision_id: u64,
        choice: Option<Operation>,
        target: Option<&str>,
    ) -> Result<(), String> {
        let decision = self.awaiting_decision(decision_id)?;
        if choice
            .as_ref()
            .is_some_and(|op| !decision.legal.contains(op))
        {
            return Err("policy returned an illegal operation".into());
        }
        if choice.is_none() && decision.point != DecisionPoint::Insert {
            return Err("turn policy declined all legal actions".into());
        }
        let point = decision.point;
        let primary_enemy = if let (Some(op), Some(target)) = (&choice, target) {
            let actor = self.state.runtime_actor_index(op.actor);
            let mut matches = self
                .state
                .primary_enemy_candidates(actor, op.action)?
                .filter(|(_, name)| *name == target);
            let (index, _) = matches.next().ok_or_else(|| {
                format!("primary target {target:?} is not an active living enemy")
            })?;
            if matches.next().is_some() {
                return Err(format!("ambiguous primary target {target:?}"));
            }
            Some(index)
        } else {
            None
        };
        let result = self.execute_choice(point, choice, primary_enemy);
        if let Err(error) = result {
            self.phase = Phase::Failed(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// 取出完成结果。尚在运行或等待决策时返回错误；失败返回原始原因。
    pub fn into_result(self) -> Result<BattleResult, String> {
        match self.phase {
            Phase::Finished(result) => Ok(*result),
            Phase::Failed(error) => Err(error),
            _ => Err("battle has not finished".into()),
        }
    }

    /// 执行普通或额外回合选择，插入窗口只登记终结技。
    fn execute_choice(
        &mut self,
        point: DecisionPoint,
        choice: Option<Operation>,
        primary_enemy: Option<usize>,
    ) -> Result<(), String> {
        let choice = choice.map(|op| Operation {
            actor: self.state.runtime_actor_index(op.actor),
            ..op
        });
        if point == DecisionPoint::Insert {
            if let Some(op) = choice {
                self.state.enqueue(Pending {
                    actor: op.actor,
                    kind: RequestKind::Ultimate,
                    action: Some(op.action),
                    primary_enemy,
                })?;
            }
            self.phase = Phase::AfterInsert;
        } else if let Some(op) = choice {
            self.state.act(op.actor, op.action, false, primary_enemy)?;
            if point == DecisionPoint::ExtraTurn {
                self.state
                    .dispatch(BattleEvent::TurnEnd, Some(op.actor), None)?;
                trace!(
                    self.state,
                    "extra_turn_end",
                    op.actor,
                    None,
                    None,
                    None,
                    None
                );
            }
            self.phase = Phase::Boundary;
        }
        Ok(())
    }

    /// 创建当前决策并递增会话内编号。
    fn pause(&mut self, point: DecisionPoint, actor: Option<usize>) -> Result<(), String> {
        let id = self.next_decision_id;
        self.next_decision_id = id.checked_add(1).ok_or("decision sequence exhausted")?;
        self.phase = Phase::Awaiting(self.state.decision(point, actor, id));
        Ok(())
    }

    /// 保存完成结果并结束本场推进。
    fn conclude(&mut self, outcome: Outcome, termination: Termination) {
        // A terminal or safety stop can interrupt a natural turn without running TurnEnd rules.
        #[cfg(feature = "diagnostics")]
        if let Some(actor) = self
            .state
            .turn
            .filter(|&actor| self.state.units[actor].summon.is_some())
        {
            trace!(
                self.state,
                "normal_turn_interrupted",
                actor,
                None,
                None,
                None,
                None
            );
        }
        let cleared = self.state.wave + usize::from(!self.state.enemies_present());
        self.phase = Phase::Finished(Box::new(self.state.finish(outcome, termination, cleared)));
    }

    /// 清场/全灭独立触发结算；未来波次或尚未入场的队列仍参与敌方存在判断。
    fn settle_elimination(&mut self) -> bool {
        let allies = self.state.allies_present();
        let enemies = self.state.enemies_present();
        let end = match (allies, enemies) {
            (false, false) => Some((Outcome::Draw, Termination::BothDefeated)),
            (false, true) => Some((Outcome::Defeat, Termination::AlliesDefeated)),
            (true, false) if self.state.wave + 1 == self.state.input.waves.len() => {
                Some((Outcome::Victory, Termination::EnemiesDefeated))
            }
            _ => None,
        };
        if let Some((outcome, termination)) = end {
            self.conclude(outcome, termination);
            true
        } else {
            false
        }
    }

    /// 分派普通回合结束事件并重置距离、应用下次提前。
    fn end_normal_turn(&mut self, actor: usize) -> Result<(), String> {
        self.state
            .dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
        if self.state.units[actor].summon.is_none() || self.state.units[actor].alive() {
            self.state.units[actor].remaining =
                (DISTANCE - self.state.units[actor].next_shift).max(0.0);
        }
        self.state.units[actor].next_shift = 0.0;
        self.state.turn = None;
        trace!(
            self.state,
            "normal_turn_end",
            actor,
            None,
            None,
            Some(self.state.units[actor].remaining),
            None,
        );
        Ok(())
    }

    /// 按战斗边界推进换波、倒计时、插入及普通行动，直到暂停或结束。
    fn advance_inner(&mut self, cancel: &mut impl FnMut() -> bool) -> Result<(), String> {
        let input = self.state.input;
        loop {
            if matches!(self.phase, Phase::Boundary) {
                if self.settle_elimination() {
                    return Ok(());
                }
                if !self.state.enemies_present() {
                    if self.state.wave == 0 {
                        self.state.expire_opening_battle_event();
                    }
                    self.state.wave += 1;
                    for u in &input.waves[self.state.wave] {
                        self.state
                            .units
                            .push(CombatUnit::new(u, false, &input.mechanics));
                    }
                    self.state.dispatch(BattleEvent::WaveStart, None, None)?;
                    continue;
                }
                // 负责人确认（2026-11-17）：清场/全灭与倒计时回合结束是独立
                // 结算触发。额度归零时，已进入的单位Turn先完成收尾。
                if self.state.timeline.remaining == 0.0 {
                    if let Some(actor) = self.state.turn {
                        // 完整Action后的补位先于TurnEnd，也先于新入场单位的插入检查。
                        self.state.refill_queue();
                        let insert_first = input.mechanics.unit_before_cycle
                            && (!self.state.pending.is_empty()
                                || !self
                                    .state
                                    .legal_operations(DecisionPoint::Insert, None)
                                    .is_empty());
                        if !insert_first {
                            self.end_normal_turn(actor)?;
                            continue;
                        }
                    }
                }
                if self.state.exhausted {
                    self.conclude(Outcome::Defeat, Termination::AvBudgetExhausted);
                    return Ok(());
                }
                let zero_time_due = input.mechanics.unit_before_cycle
                    && (self.state.turn.is_some()
                        || !self.state.pending.is_empty()
                        || self
                            .state
                            .units
                            .iter()
                            .any(|u| u.alive() && u.remaining == 0.0));
                if self.state.timeline.remaining == 0.0 && !zero_time_due {
                    trace!(
                        self.state,
                        "cycle_end",
                        0,
                        None,
                        None,
                        Some((self.state.timeline.cycle + 1) as f64),
                        None,
                    );
                    self.state.dispatch(BattleEvent::CycleEnd, None, None)?;
                    if !self.state.timeline.next_cycle() {
                        self.state.exhausted = true;
                    }
                    continue;
                }
                if cancel() {
                    self.conclude(Outcome::Incomplete, Termination::Cancelled);
                    return Ok(());
                }
                if self.state.actions >= input.max_actions {
                    self.conclude(Outcome::Incomplete, Termination::ActionLimit);
                    return Ok(());
                }
                self.state.refill_queue();
                self.pause(DecisionPoint::Insert, None)?;
                return Ok(());
            }
            if !matches!(self.phase, Phase::AfterInsert) {
                return Ok(());
            }
            self.phase = Phase::Boundary;
            if let Some(request) = self.state.pending.pop_front() {
                let actor = request.actor;
                if !self.state.can_execute_request(actor) {
                    trace!(self.state, "request_invalid", actor, None, None, None, None);
                    continue;
                }
                if request.kind == RequestKind::ExtraTurn {
                    trace!(
                        self.state,
                        "extra_turn_start",
                        actor,
                        None,
                        None,
                        None,
                        None
                    );
                    self.state
                        .dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                    if !self.state.units[actor].alive() {
                        continue;
                    }
                    self.pause(DecisionPoint::ExtraTurn, Some(actor))?;
                    return Ok(());
                }
                let action = request.action.ok_or("request has no preselected action")?;
                if let Some(_target) = request
                    .primary_enemy
                    .filter(|&target| !self.state.is_primary_enemy(actor, target))
                {
                    trace!(
                        self.state,
                        "request_target_invalid",
                        actor,
                        Some(_target),
                        None,
                        None,
                        None
                    );
                    continue;
                }
                if !self
                    .state
                    .affordable(actor, &self.state.units[actor].spec.actions[action])
                {
                    trace!(self.state, "request_invalid", actor, None, None, None, None);
                    continue;
                }
                if request.kind == RequestKind::Ultimate {
                    let skill = self.state.units[actor].spec.actions[action].clone();
                    self.state.pay(actor, &skill);
                    trace!(
                        self.state,
                        "ultimate_turn_start",
                        actor,
                        None,
                        None,
                        Some(self.state.units[actor].energy),
                        None,
                    );
                    self.state
                        .dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                    if !self.state.units[actor].alive() {
                        continue;
                    }
                }
                if let Some(_target) = request
                    .primary_enemy
                    .filter(|&target| !self.state.is_primary_enemy(actor, target))
                {
                    // TurnStart listeners can remove a target after entry payment.
                    trace!(
                        self.state,
                        "request_target_invalid",
                        actor,
                        Some(_target),
                        None,
                        None,
                        None
                    );
                } else {
                    self.state.act(
                        actor,
                        action,
                        request.kind == RequestKind::Ultimate,
                        request.primary_enemy,
                    )?;
                }
                if request.kind != RequestKind::FollowUp {
                    self.state
                        .dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
                    trace!(self.state, "extra_turn_end", actor, None, None, None, None);
                }
                continue;
            }
            if let Some(actor) = self.state.turn {
                self.end_normal_turn(actor)?;
                continue;
            }
            let (actor, wait) = scheduler::next_normal(
                self.state
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.alive())
                    .map(|(i, u)| (i, u.remaining, u.spec.stats.speed)),
            )
            .ok_or("empty axis")?;
            let cycle_first = self.state.timeline.remaining < wait
                || (self.state.timeline.remaining == wait && !input.mechanics.unit_before_cycle);
            let delta = wait.min(self.state.timeline.remaining);
            self.state.timeline.advance(delta);
            for u in &mut self.state.units {
                if u.alive() {
                    u.remaining = (u.remaining - delta * u.spec.stats.speed).max(0.0);
                }
            }
            if cycle_first {
                continue;
            }
            self.state.turn = Some(actor);
            trace!(
                self.state,
                "normal_turn_start",
                actor,
                None,
                None,
                Some(self.state.units[actor].remaining),
                None,
            );
            if self.state.units[actor].shield_turns > 0 {
                self.state.units[actor].shield_turns -= 1;
                if self.state.units[actor].shield_turns == 0 {
                    #[cfg(feature = "diagnostics")]
                    let expired = self.state.units[actor].shield;
                    self.state.units[actor].shield = 0.0;
                    trace!(
                        self.state,
                        "shield_expired",
                        actor,
                        Some(actor),
                        None,
                        Some(expired),
                        None,
                    );
                }
            }
            self.state
                .dispatch(BattleEvent::TurnStart, Some(actor), None)?;
            if self.state.units[actor].summon.is_some() {
                if self.state.units[actor].alive() {
                    self.state.act(actor, 0, false, None)?;
                }
                // Resume the same refill/insert/pending/TurnEnd path as a chosen action.
                continue;
            }
            if !self.state.units[actor].alive() {
                continue;
            }
            self.pause(DecisionPoint::NormalTurn, Some(actor))?;
            return Ok(());
        }
    }
}
