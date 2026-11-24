//! 战斗拥有的可变状态、单位入场与有限队列。
use super::*;

/// 单位的本场可变状态，初始定义在每场开始时复制。
pub(super) struct CombatUnit {
    /// 原始定义的本场副本。
    pub(super) spec: Unit,
    /// 是否属于我方。
    pub(super) ally: bool,
    /// Original ally index and whether the summon is still in battle.
    pub(super) summon: Option<SummonPresence>,
    /// 当前生命值。
    pub(super) hp: f64,
    /// 当前护盾容量。
    pub(super) shield: f64,
    /// 护盾剩余普通回合计数。
    pub(super) shield_turns: u16,
    /// 当前能量。
    pub(super) energy: f64,
    /// 距下次普通行动的剩余距离。
    pub(super) remaining: f64,
    /// 普通 Turn 结束重排时应用的距离调整。
    pub(super) next_shift: f64,
    /// 单位当前韧性或按单位关联的初始韧性。
    pub(super) toughness: f64,
    /// 单位持久变量表或按单位关联的初始变量。
    pub(super) dynamic_values: std::collections::BTreeMap<String, f64>,
    /// 当前活动属性状态。
    pub(super) modifiers: ModifierBook,
    /// 本场剩余即时复活次数。
    pub(super) revivals_left: u8,
}
/// Lifecycle of an opening traditional summon.
pub(super) struct SummonPresence {
    /// Stable index of its owner in the starting roster.
    pub(super) owner: usize,
    /// False after the owner has been defeated and revival has resolved.
    pub(super) present: bool,
}

impl CombatUnit {
    /// 按所属类型的初始配置建立独立运行状态。
    pub(super) fn new(spec: &Unit, ally: bool, mechanics: &Mechanics) -> Self {
        Self {
            spec: spec.clone(),
            ally,
            summon: None,
            hp: spec.initial_hp,
            shield: 0.0,
            shield_turns: 0,
            energy: spec.initial_energy,
            remaining: DISTANCE,
            next_shift: 0.0,
            toughness: mechanics.toughness.get(&spec.id).copied().unwrap_or(0.0),
            dynamic_values: mechanics
                .dynamic_values
                .get(&spec.id)
                .cloned()
                .unwrap_or_default(),
            modifiers: ModifierBook::default(),
            revivals_left: spec.revival.as_ref().map_or(0, |r| r.charges),
        }
    }
    /// Departed summons remain in the unit vector only for observations and results.
    pub(super) fn present(&self) -> bool {
        self.summon.as_ref().is_none_or(|summon| summon.present)
    }
    /// A departed summon retains its last panel but cannot participate.
    pub(super) fn alive(&self) -> bool {
        self.hp > 0.0 && self.present()
    }
    /// Regular living units can be selected by generic side-based selectors.
    pub(super) fn regular_alive(&self) -> bool {
        self.summon.is_none() && self.alive()
    }
    /// 按 scale 取得攻击、防御或生命上限，供效果合成基础量。
    pub(super) fn scale(&self, scale: Scale) -> f64 {
        match scale {
            Scale::Attack => self.spec.stats.attack,
            Scale::Defense => self.spec.stats.defense,
            Scale::MaxHp => self.spec.stats.max_hp,
        }
    }
    /// 导出当前单位快照；观测和最终结果共用相同字段。
    fn snapshot(
        &self,
        weaknesses: Option<crate::weakness::WeaknessState>,
        summon_owner: Option<String>,
    ) -> UnitResult {
        UnitResult {
            weaknesses,
            id: self.spec.id.clone(),
            ally: self.ally,
            hp: self.hp,
            shield: self.shield,
            energy: self.energy,
            summon_owner,
            alive: self.alive(),
            revivals_left: self.revivals_left,
            remaining_distance: self.remaining,
            speed: self.spec.stats.speed,
            toughness: self.toughness,
            dynamic_values: self.dynamic_values.clone(),
            modifiers: self.modifiers.states(),
        }
    }
}
#[derive(Clone)]
/// 完整动作结束后等待调度的插入请求。
pub(super) struct Pending {
    /// 本场执行者索引或回放中的执行者 ID。
    pub(super) actor: usize,
    /// 本次请求的调度方式。
    pub(super) kind: RequestKind,
    /// 可选动作标识或该执行者动作列表中的索引。
    pub(super) action: Option<usize>,
    /// Validated runtime instance selected when the request was registered.
    pub(super) primary_enemy: Option<usize>,
}
/// 一条监听规则在本场的剩余次数与寿命。
pub(super) struct Hook {
    /// 原始定义的本场副本。
    pub(super) spec: Trigger,
    /// 剩余触发次数，None 表示无限，Some(0) 表示停用。
    pub(super) remaining: Option<u32>,
    /// 剩余或初始寿命事件计数。
    pub(super) duration: u32,
}
/// 本场单位、资源、时钟、监听器和随机流的统一存储。
pub(super) struct State<'a> {
    /// 借用的组装后配置。
    pub(super) input: &'a Scenario,
    /// 已经入场的单位，死亡记录保留。
    pub(super) units: Vec<CombatUnit>,
    /// 本场时钟及各轮剩余额度。
    pub(super) timeline: Timeline,
    /// 按登记顺序排队的后续动作。
    pub(super) pending: std::collections::VecDeque<Pending>,
    /// 独立开场事件在执行者表中的索引。
    opening_event_actor: Option<usize>,
    /// 正在进行的普通 Turn 执行者。
    pub(super) turn: Option<usize>,
    /// 本场监听器的剩余次数和寿命。
    pub(super) hooks: Vec<Hook>,
    /// 嵌套事件的当前深度。
    pub(super) event_depth: u32,
    /// 嵌套效果的当前深度。
    pub(super) effect_depth: u32,
    /// 当前顶层效果调用链已执行的节点数。
    pub(super) effect_steps: u32,
    /// 当前完整动作的类别。
    pub(super) current_action_kind: Option<ActionKind>,
    /// 最终 Cycle 已结束且额度耗尽的标记。
    pub(super) exhausted: bool,
    /// Action 入口绑定的技能目标。
    pub(super) bound_target: Option<usize>,
    /// 临时重选作用域中的目标。
    pub(super) context_target: Option<usize>,
    /// 已命中目标列表所属的动作执行者。
    pub(super) attack_owner: Option<usize>,
    /// 当前动作实际命中过的不同单位。
    pub(super) attack_targets: Vec<usize>,
    /// 已执行的完整 Action 数。
    pub(super) actions: u32,
    /// 当前敌方波次的内部索引。
    pub(super) wave: usize,
    /// 有限队列下一位待入场敌人的位置。
    pub(super) queue_next: usize,
    /// 当前队列槽位到单位索引的映射。
    pub(super) queue_slots: Vec<Option<usize>>,
    /// 双方当前战技点，零索引为我方。
    pub(super) sp: [u8; 2],
    #[cfg(feature = "diagnostics")]
    /// 是否启用本场诊断回放。
    pub(super) trace: bool,
    #[cfg(feature = "diagnostics")]
    /// 按执行顺序保存的诊断回放。
    pub(super) events: Vec<crate::diagnostics::Event>,
    #[cfg(feature = "diagnostics")]
    /// 诊断回放是否达到条数上限。
    pub(super) trace_truncated: bool,
    /// 目标选择专用的独立随机流。
    pub(super) targets: ChaCha8Rng,
    /// 暴击专用随机流。
    pub(super) criticals: ChaCha8Rng,
    /// 元素选择专用随机流。
    pub(super) elements: ChaCha8Rng,
    /// 当前伤害的显式元素上下文。
    pub(super) damage_element: Option<Element>,
    /// 正在计算的伤害类别。
    pub(super) damage_attack_type: Option<AttackType>,
    /// 当前事件关联的命中类别。
    pub(super) event_attack_type: Option<AttackType>,
    /// 能力与任务模板临时变量帧栈。
    pub(super) dynamic_scopes: crate::dynamic_scope::ScopeFrames,
}

impl<'a> State<'a> {
    /// 按所属类型的初始配置建立独立运行状态。
    pub(super) fn new(
        input: &'a Scenario,
        seed: u64,
        #[cfg(feature = "diagnostics")] trace: bool,
    ) -> Self {
        let s = input;
        let mut units: Vec<_> = s
            .allies
            .iter()
            .map(|u| CombatUnit::new(u, true, &s.mechanics))
            .collect();
        let initial_count = s
            .mechanics
            .queue_capacity
            .unwrap_or(s.waves[0].len())
            .min(s.waves[0].len());
        units.extend(
            s.waves[0]
                .iter()
                .take(initial_count)
                .map(|u| CombatUnit::new(u, false, &s.mechanics)),
        );
        Self {
            input,
            units,
            timeline: Timeline::new(if s.mechanics.cycles.is_empty() {
                vec![s.max_av]
            } else {
                s.mechanics.cycles.clone()
            }),
            pending: Default::default(),
            turn: None,
            hooks: s
                .mechanics
                .triggers
                .iter()
                .map(|t| Hook {
                    spec: t.clone(),
                    remaining: if t.uses == 0 { None } else { Some(t.uses) },
                    duration: t.duration,
                })
                .collect(),
            event_depth: 0,
            opening_event_actor: None,
            effect_depth: 0,
            effect_steps: 0,
            current_action_kind: None,
            exhausted: false,
            bound_target: None,
            context_target: None,
            attack_owner: None,
            attack_targets: vec![],
            actions: 0,
            wave: 0,
            queue_next: initial_count,
            queue_slots: (0..s.mechanics.queue_capacity.unwrap_or(0))
                .map(|i| (i < initial_count).then_some(s.allies.len() + i))
                .collect(),
            sp: [s.ally_skill_points.initial, s.enemy_skill_points.initial],
            #[cfg(feature = "diagnostics")]
            trace,
            #[cfg(feature = "diagnostics")]
            events: Vec::new(),
            #[cfg(feature = "diagnostics")]
            trace_truncated: false,
            targets: stream(seed, b"targets"),
            criticals: stream(seed, b"criticals"),
            elements: stream(seed, b"elements"),
            damage_element: None,
            damage_attack_type: None,
            event_attack_type: None,
            dynamic_scopes: Default::default(),
        }
    }

    /// Instantiate opening summons after the normal roster, preserving queue slot indexes.
    pub(super) fn spawn_opening_summons(&mut self) -> Result<(), String> {
        let summons = &self.input.mechanics.traditional_summons;
        if summons.is_empty() {
            return Ok(());
        }
        let mut ids: std::collections::HashSet<_> = self
            .input
            .allies
            .iter()
            .chain(self.input.waves.iter().flatten())
            .map(|unit| unit.id.as_str())
            .collect();
        for summon in summons {
            let id = &summon.id;
            if id.is_empty() || !ids.insert(id.as_str()) {
                return Err(format!(
                    "invalid or duplicate traditional summon ID: {id:?}"
                ));
            }
            let mut matching_owners = self
                .input
                .allies
                .iter()
                .enumerate()
                .filter(|(_, unit)| unit.id == summon.owner);
            let (owner, _) = matching_owners.next().ok_or_else(|| {
                format!(
                    "traditional summon {id} has unknown ally owner {}",
                    summon.owner
                )
            })?;
            if matching_owners.next().is_some() {
                return Err(format!(
                    "traditional summon {id} has ambiguous ally owner {}",
                    summon.owner
                ));
            }
            let action = &summon.action;
            if action.kind != ActionKind::FollowUp
                || action.sp_cost != 0
                || action.sp_gain != 0
                || action.energy_cost != 0.0
                || action.energy_gain != 0.0
            {
                return Err(format!(
                    "traditional summon {id} needs a resource-free follow-up"
                ));
            }
            let stats = &summon.stats;
            if !stats.speed.is_finite() || stats.speed <= 0.0 {
                return Err(format!(
                    "traditional summon {id} needs finite positive speed"
                ));
            }
            if !stats.max_hp.is_finite() || stats.max_hp <= 0.0 {
                return Err(format!(
                    "traditional summon {id} needs finite positive max_hp"
                ));
            }
            if !self.units[owner].alive() {
                continue;
            }
            let unit = Unit {
                id: id.clone(),
                stats: stats.clone(),
                initial_hp: stats.max_hp,
                initial_energy: 0.0,
                max_energy: 0.0,
                revival: None,
                actions: vec![action.clone()],
            };
            let mut unit = CombatUnit::new(&unit, true, &self.input.mechanics);
            unit.summon = Some(SummonPresence {
                owner,
                present: true,
            });
            let _index = self.units.len();
            self.units.push(unit);
            trace!(self, "summon_spawn", _index, Some(owner), None, None, None);
        }
        Ok(())
    }

    /// 在开场监听器处理完毕后登记独立战斗事件。
    pub(super) fn queue_opening_battle_event(&mut self) -> Result<(), String> {
        let Some(event) = &self.input.mechanics.opening_battle_event else {
            return Ok(());
        };
        let id = event.id.as_str();
        let mut ids = self
            .input
            .allies
            .iter()
            .chain(self.input.waves.iter().flatten())
            .map(|unit| unit.id.as_str())
            .chain(
                self.input
                    .mechanics
                    .traditional_summons
                    .iter()
                    .map(|summon| summon.id.as_str()),
            );
        if id.is_empty() || ids.any(|unit_id| unit_id == id) {
            return Err(format!(
                "invalid or duplicate opening battle event ID: {id:?}"
            ));
        }
        let action = &event.action;
        if action.kind != ActionKind::FollowUp
            || action.sp_cost != 0
            || action.sp_gain != 0
            || action.energy_cost != 0.0
            || action.energy_gain != 0.0
        {
            return Err(format!(
                "opening battle event {id} needs a resource-free follow-up"
            ));
        }
        if !self.allies_present() || !self.enemies_present() {
            return Ok(());
        }

        // The shared action code needs a stable actor index. With no life pool,
        // the event is excluded from combatant targeting and the natural axis.
        let source = Unit {
            id: event.id.clone(),
            stats: event.stats.clone(),
            initial_hp: 0.0,
            initial_energy: 0.0,
            max_energy: 0.0,
            revival: None,
            actions: vec![action.clone()],
        };
        let actor = self.units.len();
        self.units
            .push(CombatUnit::new(&source, true, &self.input.mechanics));
        self.opening_event_actor = Some(actor);
        self.enqueue(Pending {
            actor,
            kind: RequestKind::FollowUp,
            action: Some(0),
            primary_enemy: None,
        })
    }

    /// 区分独立伤害来源与参战单位。
    pub(super) fn is_opening_event(&self, actor: usize) -> bool {
        self.opening_event_actor == Some(actor)
    }

    /// 将执行者索引转换为公开单位列表中的位置。
    pub(super) fn observed_actor_index(&self, actor: usize) -> usize {
        actor - usize::from(self.opening_event_actor.is_some_and(|event| event < actor))
    }

    /// 将已核对的公开操作索引还原为内部执行者位置。
    pub(super) fn runtime_actor_index(&self, actor: usize) -> usize {
        actor + usize::from(self.opening_event_actor.is_some_and(|event| event <= actor))
    }

    /// 首波结束时清除尚未执行的开场请求，保留其他已登记动作。
    pub(super) fn expire_opening_battle_event(&mut self) {
        if let Some(actor) = self.opening_event_actor {
            self.pending.retain(|request| request.actor != actor);
        }
    }

    /// 开场事件的单次预登记请求仅能在首波执行。
    pub(super) fn can_execute_request(&self, actor: usize) -> bool {
        if self.is_opening_event(actor) {
            self.wave == 0
        } else {
            self.units[actor].alive()
        }
    }

    /// 当前仍有参战队员。
    pub(super) fn allies_present(&self) -> bool {
        self.units
            .iter()
            .any(|unit| unit.ally && unit.regular_alive())
    }

    /// 当前仍有敌方单位或本波有限队列中的替补。
    pub(super) fn enemies_present(&self) -> bool {
        let queued = self.input.mechanics.queue_capacity.is_some()
            && self.queue_next < self.input.waves[0].len();
        queued || self.units.iter().any(|unit| !unit.ally && unit.alive())
    }

    /// 返回显式命中类别或来源的默认类别。
    pub(super) fn hit_attack_type(&self, actor: usize) -> AttackType {
        self.damage_attack_type
            .unwrap_or(if self.is_opening_event(actor) {
                AttackType::Insert
            } else {
                AttackType::Direct
            })
    }

    /// Retain departed instances for snapshots and invalidate their pending requests.
    pub(super) fn dismiss_summons(&mut self, owner: usize) {
        if self.input.mechanics.traditional_summons.is_empty() {
            return;
        }
        for index in 0..self.units.len() {
            let Some(summon) = &mut self.units[index].summon else {
                continue;
            };
            if summon.owner == owner && summon.present {
                summon.present = false;
                trace!(self, "summon_despawn", index, Some(owner), None, None, None);
            }
        }
        self.pending.retain(|request| {
            self.units[request.actor]
                .summon
                .as_ref()
                .is_none_or(|summon| summon.present)
        });
    }

    /// Build the same unit view for any observation or battle result.
    fn unit_snapshot(&self, index: usize) -> UnitResult {
        let unit = &self.units[index];
        let owner = unit
            .summon
            .as_ref()
            .map(|summon| self.units[summon.owner].spec.id.clone());
        unit.snapshot(self.weakness_state(index), owner)
    }

    /// 公开的单位列表只包含参战单位和已入场召唤物。
    pub(super) fn unit_snapshots(&self) -> Vec<UnitResult> {
        (0..self.units.len())
            .filter(|&index| !self.is_opening_event(index))
            .map(|index| self.unit_snapshot(index))
            .collect()
    }

    /// Reuse slots only after the previous complete Action. Revival has already resolved.
    pub(super) fn refill_queue(&mut self) {
        if self.input.mechanics.queue_capacity.is_none() {
            return;
        }
        let queue = &self.input.waves[0];
        for slot in 0..self.queue_slots.len() {
            if self.queue_slots[slot].is_some_and(|i| self.units[i].alive()) {
                continue;
            }
            self.queue_slots[slot] = None;
            if self.queue_next == queue.len() {
                continue;
            }
            let spec = &queue[self.queue_next];
            let unit = CombatUnit::new(spec, false, &self.input.mechanics);
            let index = self.units.len();
            self.units.push(unit);
            self.queue_slots[slot] = Some(index);
            self.queue_next += 1;
            trace!(
                self,
                "queue_spawn",
                index,
                None,
                None,
                Some(slot as f64),
                None
            );
        }
    }

    /// 返回执行者所属资源侧的索引，零为我方。
    pub(super) fn side(&self, actor: usize) -> usize {
        usize::from(!self.units[actor].ally)
    }

    /// 按资源侧索引取得共享战技点上限。
    pub(super) fn sp_max(&self, side: usize) -> u8 {
        if side == 0 {
            self.input.ally_skill_points.maximum
        } else {
            self.input.enemy_skill_points.maximum
        }
    }

    /// 根据当前战技点与执行者能量判断费用是否可支付。
    pub(super) fn affordable(&self, actor: usize, action: &Action) -> bool {
        action.sp_cost <= self.sp[self.side(actor)]
            && action.energy_cost <= self.units[actor].energy
    }
}
