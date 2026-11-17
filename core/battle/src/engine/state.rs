//! 战斗拥有的可变状态、单位入场与有限队列。
use super::*;

pub(super) struct CombatUnit {
    pub(super) spec: Unit,
    pub(super) ally: bool,
    pub(super) hp: f64,
    pub(super) shield: f64,
    pub(super) shield_turns: u16,
    pub(super) energy: f64,
    pub(super) remaining: f64,
    pub(super) next_shift: f64,
    pub(super) toughness: f64,
    pub(super) dynamic_values: std::collections::BTreeMap<String, f64>,
    pub(super) modifiers: ModifierBook,
    pub(super) revivals_left: u8,
}
impl CombatUnit {
    pub(super) fn new(spec: &Unit, ally: bool, mechanics: &Mechanics) -> Self {
        Self {
            spec: spec.clone(),
            ally,
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
    pub(super) fn alive(&self) -> bool {
        self.hp > 0.0
    }
    pub(super) fn scale(&self, scale: Scale) -> f64 {
        match scale {
            Scale::Attack => self.spec.stats.attack,
            Scale::Defense => self.spec.stats.defense,
            Scale::MaxHp => self.spec.stats.max_hp,
        }
    }
    /// 导出当前单位快照；观测和最终结果共用相同字段。
    pub(super) fn snapshot(
        &self,
        weaknesses: Option<crate::weakness::WeaknessState>,
    ) -> UnitResult {
        UnitResult {
            weaknesses,
            id: self.spec.id.clone(),
            ally: self.ally,
            hp: self.hp,
            shield: self.shield,
            energy: self.energy,
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
pub(super) struct Pending {
    pub(super) actor: usize,
    pub(super) kind: RequestKind,
    pub(super) action: Option<usize>,
}
pub(super) struct Hook {
    pub(super) spec: Trigger,
    pub(super) remaining: Option<u32>,
    pub(super) duration: u32,
}
pub(super) struct State<'a> {
    pub(super) input: &'a ValidatedScenario,
    pub(super) units: Vec<CombatUnit>,
    pub(super) timeline: Timeline,
    pub(super) pending: std::collections::VecDeque<Pending>,
    pub(super) turn: Option<usize>,
    pub(super) hooks: Vec<Hook>,
    pub(super) event_depth: u32,
    pub(super) effect_depth: u32,
    pub(super) effect_steps: u32,
    pub(super) current_action_kind: Option<ActionKind>,
    pub(super) exhausted: bool,
    pub(super) bound_target: Option<usize>,
    pub(super) context_target: Option<usize>,
    pub(super) attack_owner: Option<usize>,
    pub(super) attack_targets: Vec<usize>,
    pub(super) actions: u32,
    pub(super) wave: usize,
    pub(super) queue_next: usize,
    pub(super) queue_slots: Vec<Option<usize>>,
    pub(super) sp: [u8; 2],
    pub(super) trace: bool,
    pub(super) events: Vec<Event>,
    pub(super) trace_truncated: bool,
    pub(super) targets: ChaCha8Rng,
    pub(super) criticals: ChaCha8Rng,
    pub(super) elements: ChaCha8Rng,
    pub(super) damage_element: Option<Element>,
    pub(super) damage_attack_type: Option<AttackType>,
    pub(super) event_attack_type: Option<AttackType>,
    pub(super) dynamic_scopes: crate::dynamic_scope::ScopeFrames,
}

impl<'a> State<'a> {
    pub(super) fn new(input: &'a ValidatedScenario, seed: u64, trace: bool) -> Self {
        let s = input.scenario();
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
            trace,
            events: Vec::new(),
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

    /// Reuse slots only after the previous complete Action. Revival has already resolved.
    pub(super) fn refill_queue(&mut self) {
        if self.input.scenario().mechanics.queue_capacity.is_none() {
            return;
        }
        let queue = &self.input.scenario().waves[0];
        for slot in 0..self.queue_slots.len() {
            if self.queue_slots[slot].is_some_and(|i| self.units[i].alive()) {
                continue;
            }
            self.queue_slots[slot] = None;
            if self.queue_next == queue.len() {
                continue;
            }
            let spec = &queue[self.queue_next];
            let unit = CombatUnit::new(spec, false, &self.input.scenario().mechanics);
            let index = self.units.len();
            self.units.push(unit);
            self.queue_slots[slot] = Some(index);
            self.queue_next += 1;
            self.emit("queue_spawn", index, None, None, Some(slot as f64), None);
        }
    }

    pub(super) fn side(&self, actor: usize) -> usize {
        usize::from(!self.units[actor].ally)
    }

    pub(super) fn sp_max(&self, side: usize) -> u8 {
        if side == 0 {
            self.input.scenario().ally_skill_points.maximum
        } else {
            self.input.scenario().enemy_skill_points.maximum
        }
    }

    pub(super) fn affordable(&self, actor: usize, action: &Action) -> bool {
        action.sp_cost <= self.sp[self.side(actor)]
            && action.energy_cost <= self.units[actor].energy
    }
}
