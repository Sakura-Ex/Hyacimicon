//! Single-owner event loop for the explicit battle_v2 contract.
use crate::{
    decision::*,
    elements::{DamageElement, Element},
    math,
    model::*,
    modifiers::{ModifierBook, Property},
    scheduler::{self, Timeline, DISTANCE},
};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

pub const RNG_ALGORITHM: &str =
    "ChaCha8Rng/rand_chacha=0.9.0; SHA256-separated target/critical/element streams v1";
const TRACE_LIMIT: usize = 100_000;

struct CombatUnit {
    spec: Unit,
    ally: bool,
    hp: f64,
    shield: f64,
    shield_turns: u16,
    energy: f64,
    remaining: f64,
    next_shift: f64,
    toughness: f64,
    dynamic_values: std::collections::BTreeMap<String, f64>,
    modifiers: ModifierBook,
    revivals_left: u8,
}
impl CombatUnit {
    fn new(spec: &Unit, ally: bool, _now: f64) -> Self {
        Self {
            spec: spec.clone(),
            ally,
            hp: spec.initial_hp,
            shield: 0.0,
            shield_turns: 0,
            energy: spec.initial_energy,
            remaining: DISTANCE,
            next_shift: 0.0,
            toughness: 0.0,
            dynamic_values: Default::default(),
            modifiers: ModifierBook::default(),
            revivals_left: spec.revival.as_ref().map_or(0, |r| r.charges),
        }
    }
    fn alive(&self) -> bool {
        self.hp > 0.0
    }
    fn scale(&self, scale: Scale) -> f64 {
        match scale {
            Scale::Attack => self.spec.stats.attack,
            Scale::Defense => self.spec.stats.defense,
            Scale::MaxHp => self.spec.stats.max_hp,
        }
    }
}
#[derive(Clone)]
struct Pending {
    actor: usize,
    kind: RequestKind,
    action: Option<usize>,
}
struct Hook {
    spec: Trigger,
    remaining: Option<u32>,
    duration: u32,
}
struct State<'a> {
    input: &'a ValidatedScenario,
    units: Vec<CombatUnit>,
    timeline: Timeline,
    pending: std::collections::VecDeque<Pending>,
    turn: Option<usize>,
    hooks: Vec<Hook>,
    event_depth: u32,
    effect_depth: u32,
    effect_steps: u32,
    current_action_kind: Option<ActionKind>,
    exhausted: bool,
    bound_target: Option<usize>,
    context_target: Option<usize>,
    attack_owner: Option<usize>,
    attack_targets: Vec<usize>,
    actions: u32,
    wave: usize,
    sp: [u8; 2],
    trace: bool,
    events: Vec<Event>,
    trace_truncated: bool,
    targets: ChaCha8Rng,
    criticals: ChaCha8Rng,
    elements: ChaCha8Rng,
    damage_element: Option<Element>,
    damage_attack_type: Option<AttackType>,
    event_attack_type: Option<AttackType>,
    dynamic_scopes: crate::dynamic_scope::ScopeFrames,
}
fn stream(seed: u64, domain: &[u8]) -> ChaCha8Rng {
    let mut hash = Sha256::new();
    // 保留随机流派生域，保证既有种子的回放结果一致。
    hash.update(b"hcwa-battle-core/rng/v1\0");
    hash.update(domain);
    hash.update(seed.to_le_bytes());
    ChaCha8Rng::from_seed(hash.finalize().into())
}
fn uniform(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}
fn finite(value: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite {name}"))
    }
}
impl<'a> State<'a> {
    fn pay(&mut self, actor: usize, action: &Action) {
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

    fn weakness_state(&self, owner: usize) -> Option<crate::weakness::WeaknessState> {
        let mechanics = &self.input.scenario().mechanics;
        let unit = &self.units[owner];
        let natural = mechanics.natural_weaknesses.get(&unit.spec.id)?;
        let attached = unit.modifiers.instances.iter().flat_map(|instance| {
            mechanics
                .modifier_weaknesses
                .get(&instance.state.id)
                .into_iter()
                .flatten()
                .copied()
        });
        Some(crate::weakness::WeaknessState::compose(natural, attached))
    }
    fn record_weakness_change(
        &mut self,
        owner: usize,
        before: Option<crate::weakness::WeaknessState>,
        action: Option<&str>,
    ) {
        let after = self.weakness_state(owner);
        if before != after {
            if let Some(after) = after {
                // Observation only; this is not a native OnBeingStackWeakness callback.
                self.emit(
                    "weakness_count",
                    owner,
                    Some(owner),
                    action,
                    Some(after.all.len() as f64),
                    None,
                );
            }
        }
    }
    fn dispatch(
        &mut self,
        event: BattleEvent,
        subject: Option<usize>,
        action: Option<&str>,
    ) -> Result<(), String> {
        if self.event_depth >= 64 {
            return Err("runtime event recursion exceeds 64".into());
        }
        self.event_depth += 1;
        let subject_id = subject.map(|i| self.units[i].spec.id.clone());
        let weakness_before: Vec<_> = if self
            .input
            .scenario()
            .mechanics
            .modifier_weaknesses
            .is_empty()
        {
            Vec::new()
        } else {
            (0..self.units.len())
                .map(|i| self.weakness_state(i))
                .collect()
        };
        // Settle all expiry before callbacks. Future-wave units are absent here.
        let mut expired = vec![];
        for (owner, unit) in self.units.iter_mut().enumerate() {
            for modifier in unit
                .modifiers
                .tick(event, subject_id.as_deref(), &unit.spec.id)
            {
                expired.push((owner, modifier.id));
            }
        }
        for (owner, id) in expired {
            self.emit(
                "modifier_expired",
                owner,
                Some(owner),
                Some(&id),
                None,
                None,
            );
        }
        for (owner, before) in weakness_before.into_iter().enumerate() {
            self.record_weakness_change(owner, before, action);
        }
        // Expire every applicable status before any listener can dispatch nested
        // events. Global events have no unit subject; absent future-wave owners
        // have not entered the battle and their lifetimes have not started.
        for t in &mut self.hooks {
            if t.remaining == Some(0) {
                continue;
            }
            let Some(owner) = self.units.iter().position(|u| u.spec.id == t.spec.owner) else {
                continue;
            };
            if t.spec.expires_on == Some(event) && (event.is_global() || subject == Some(owner)) {
                t.duration -= 1;
                if t.duration == 0 {
                    t.remaining = Some(0);
                }
            }
        }
        for index in 0..self.hooks.len() {
            let t = &mut self.hooks[index];
            if t.remaining == Some(0)
                || t.spec.event != event
                || t.spec
                    .subject
                    .as_ref()
                    .is_some_and(|id| Some(id) != subject_id.as_ref())
                || t.spec.action.as_deref().is_some_and(|a| Some(a) != action)
            {
                continue;
            }
            let Some(owner) = self.units.iter().position(|u| u.spec.id == t.spec.owner) else {
                continue;
            };
            // The owner's own death is the one event it receives while dead.
            // Other dead-owner listeners remain inactive, including nested deaths.
            if !self.units[owner].alive()
                && !(event == BattleEvent::Death && subject == Some(owner))
            {
                continue;
            }
            if let Some(left) = &mut t.remaining {
                *left -= 1;
            }
            let effects = t.spec.effects.clone();
            self.emit("trigger", owner, subject, action, None, None);
            for e in effects {
                self.apply_effect(owner, action.unwrap_or("event"), &e)?;
            }
        }
        self.event_depth -= 1;
        Ok(())
    }
    fn enqueue(&mut self, request: Pending) -> Result<(), String> {
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
    fn decide(
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
                .map(|(owner, u)| UnitResult {
                    weaknesses: self.weakness_state(owner),
                    id: u.spec.id.clone(),
                    ally: u.ally,
                    hp: u.hp,
                    shield: u.shield,
                    energy: u.energy,
                    alive: u.alive(),
                    revivals_left: u.revivals_left,
                    remaining_distance: u.remaining,
                    speed: u.spec.stats.speed,
                    toughness: u.toughness,
                    dynamic_values: u.dynamic_values.clone(),
                    modifiers: u.modifiers.states(),
                })
                .collect(),
        };
        let chosen = policy.decide(point, &observation, &legal);
        if chosen.as_ref().is_some_and(|op| !legal.contains(op)) {
            return Err("policy returned an illegal operation".into());
        }
        Ok(chosen)
    }
    fn new(input: &'a ValidatedScenario, seed: u64, trace: bool) -> Self {
        let s = input.scenario();
        let mut units: Vec<_> = s
            .allies
            .iter()
            .map(|u| CombatUnit::new(u, true, 0.0))
            .collect();
        units.extend(s.waves[0].iter().map(|u| CombatUnit::new(u, false, 0.0)));
        for u in &mut units {
            u.toughness = *s.mechanics.toughness.get(&u.spec.id).unwrap_or(&0.0);
            if let Some(values) = s.mechanics.dynamic_values.get(&u.spec.id) {
                u.dynamic_values = values.clone();
            }
        }
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
    fn emit(
        &mut self,
        kind: &str,
        actor: usize,
        target: Option<usize>,
        action: Option<&str>,
        amount: Option<f64>,
        critical: Option<bool>,
    ) {
        if !self.trace {
            return;
        }
        if self.events.len() >= TRACE_LIMIT {
            self.trace_truncated = true;
            return;
        }
        self.events.push(Event {
            index: self.events.len() as u32,
            av: self.timeline.now(),
            kind: kind.into(),
            actor: self.units[actor].spec.id.clone(),
            target: target.map(|i| self.units[i].spec.id.clone()),
            action: action.map(str::to_owned),
            amount,
            critical,
            attack_type: (kind == "damage")
                .then_some(self.damage_attack_type.unwrap_or(AttackType::Direct)),
        });
    }
    fn side(&self, actor: usize) -> usize {
        usize::from(!self.units[actor].ally)
    }
    fn sp_max(&self, side: usize) -> u8 {
        if side == 0 {
            self.input.scenario().ally_skill_points.maximum
        } else {
            self.input.scenario().enemy_skill_points.maximum
        }
    }
    fn affordable(&self, actor: usize, action: &Action) -> bool {
        action.sp_cost <= self.sp[self.side(actor)]
            && action.energy_cost <= self.units[actor].energy
    }
    fn targets(&mut self, actor: usize, target: Target) -> Result<Vec<usize>, String> {
        if target == Target::ContextTarget {
            let index = self
                .context_target
                .ok_or("context target used outside a retarget scope")?;
            return Ok(if self.units[index].alive() {
                vec![index]
            } else {
                vec![]
            });
        }
        if target == Target::AbilityTarget {
            return Ok(self
                .bound_target
                .into_iter()
                .filter(|&i| self.units[i].alive())
                .collect());
        }
        if target == Target::SelfUnit {
            return Ok(if self.units[actor].alive() {
                vec![actor]
            } else {
                Vec::new()
            });
        }
        let allies = matches!(target, Target::AllyLowestHp | Target::AllAllies);
        let side = self.units[actor].ally;
        let candidates: Vec<_> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive() && (u.ally == side) == allies)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        match target {
            Target::AllAllies | Target::AllEnemies => Ok(candidates),
            Target::AllyLowestHp | Target::EnemyLowestHp => {
                let selected = candidates
                    .into_iter()
                    .min_by(|&a, &b| {
                        let left = self.units[a].hp / self.units[a].spec.stats.max_hp;
                        let right = self.units[b].hp / self.units[b].spec.stats.max_hp;
                        left.total_cmp(&right).then(a.cmp(&b))
                    })
                    .ok_or("target selection unexpectedly empty")?;
                Ok(vec![selected])
            }
            Target::EnemyRandomTaunt => {
                let total: f64 = candidates
                    .iter()
                    .map(|&i| self.units[i].spec.stats.taunt)
                    .sum();
                finite(total, "taunt sum")?;
                let mut draw = uniform(&mut self.targets) * total;
                for &i in &candidates {
                    let weight = self.units[i].spec.stats.taunt;
                    if draw < weight {
                        return Ok(vec![i]);
                    }
                    draw -= weight;
                }
                // A final ulp at the upper edge belongs to the last interval.
                Ok(vec![*candidates
                    .last()
                    .ok_or("target selection unexpectedly empty")?])
            }
            Target::AbilityTarget => unreachable!("bound target handled above"),
            Target::ContextTarget => unreachable!("context target handled above"),
            Target::SelfUnit => unreachable!("self target handled before candidate selection"),
        }
    }
    // Called only by the hit that changed this unit's HP from positive to zero.
    // Do not infer a new death from HP observed after a nested listener returns.
    fn resolve_death(&mut self, target: usize, action: &str) -> Result<(), String> {
        self.emit("death", target, Some(target), Some(action), None, None);
        self.dispatch(BattleEvent::Death, Some(target), Some(action))?;
        self.units[target].shield = 0.0;
        self.units[target].shield_turns = 0;
        if self.units[target].revivals_left > 0 {
            let r = self.units[target]
                .spec
                .revival
                .as_ref()
                .ok_or("revival state missing policy")?
                .clone();
            self.units[target].revivals_left -= 1;
            self.units[target].hp = finite(
                self.units[target].spec.stats.max_hp * r.hp_fraction,
                "revival hp",
            )?;
            // Revival preserves distance and clock; only current-cycle AV is charged.
            let applied = self.timeline.penalize(r.av_penalty);
            self.emit(
                "revive",
                target,
                Some(target),
                Some(action),
                Some(self.units[target].hp),
                None,
            );
            self.emit(
                "revival_av_penalty",
                target,
                Some(target),
                Some(action),
                Some(applied),
                None,
            );
        }
        Ok(())
    }
    fn evaluate(&self, owner: usize, value: &crate::expression::Expression) -> Result<f64, String> {
        value.evaluate_with_scopes(
            &|key| self.units[owner].dynamic_values.get(key).copied(),
            &|scope, key| self.dynamic_scopes.get(owner, scope, key),
        )
    }
    fn condition(&self, actor: usize, condition: &Condition) -> Result<bool, String> {
        Ok(match condition {
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let l = self.evaluate(actor, left)?;
                let r = self.evaluate(actor, right)?;
                match comparison {
                    Comparison::Equal => l == r,
                    Comparison::NotEqual => l != r,
                    Comparison::Greater => l > r,
                    Comparison::GreaterEqual => l >= r,
                    Comparison::Less => l < r,
                    Comparison::LessEqual => l <= r,
                }
            }
            Condition::ActionKind { kind } => self.current_action_kind == Some(*kind),
            Condition::HitAttackType { attack_type } => {
                self.event_attack_type == Some(*attack_type)
            }
            Condition::Not { condition } => !self.condition(actor, condition)?,
            Condition::All { conditions } => {
                let mut yes = true;
                for c in conditions {
                    if !self.condition(actor, c)? {
                        yes = false;
                        break;
                    }
                }
                yes
            }
            Condition::Any { conditions } => {
                let mut yes = false;
                for c in conditions {
                    if self.condition(actor, c)? {
                        yes = true;
                        break;
                    }
                }
                yes
            }
        })
    }
    fn apply_effect(&mut self, actor: usize, action: &str, effect: &Effect) -> Result<(), String> {
        if self.effect_depth == 0 {
            self.effect_steps = 0;
        }
        if self.effect_depth >= 64 || self.effect_steps >= 100_000 {
            return Err("runtime effect budget exceeded".into());
        }
        self.effect_depth += 1;
        self.effect_steps += 1;
        let result = self.apply_effect_inner(actor, action, effect);
        self.effect_depth -= 1;
        result
    }
    fn apply_effect_inner(
        &mut self,
        actor: usize,
        action: &str,
        effect: &Effect,
    ) -> Result<(), String> {
        match *effect {
            Effect::WithDynamicScope { scope, ref effects } => {
                self.dynamic_scopes.push(actor, scope);
                self.emit(
                    &format!("{}_scope_enter", scope.name()),
                    actor,
                    Some(actor),
                    Some(action),
                    None,
                    None,
                );
                let result = effects
                    .iter()
                    .try_for_each(|child| self.apply_effect(actor, action, child));
                self.dynamic_scopes.pop();
                self.emit(
                    &format!("{}_scope_exit", scope.name()),
                    actor,
                    Some(actor),
                    Some(action),
                    None,
                    None,
                );
                result?;
            }
            Effect::SetScopedDynamicValue {
                scope,
                ref key,
                ref value,
            } => {
                let amount = self.evaluate(actor, value)?;
                self.dynamic_scopes.set(actor, scope, key, amount)?;
                self.emit(
                    &format!("{}_dynamic_value", scope.name()),
                    actor,
                    Some(actor),
                    Some(key),
                    Some(amount),
                    None,
                );
            }
            Effect::ElementalDamage {
                scale,
                ref ratio,
                ref flat,
                element,
                attack_type,
                can_crit,
                target,
            } => {
                let ratio = self.evaluate(actor, ratio)?;
                let flat = self.evaluate(actor, flat)?;
                if ratio < 0.0 || flat < 0.0 {
                    return Err("negative dynamic damage coefficient".into());
                }
                let element = match element {
                    DamageElement::Fixed { element } => element,
                    DamageElement::RandomAll => {
                        Element::ALL[(uniform(&mut self.elements) * 7.0) as usize]
                    }
                };
                self.emit(
                    &format!("damage_element_{}", element.name()),
                    actor,
                    self.context_target,
                    Some(action),
                    None,
                    None,
                );
                let saved = self.damage_element.replace(element);
                let saved_attack_type = self.damage_attack_type.replace(attack_type);
                let result = self.apply_effect(
                    actor,
                    action,
                    &Effect::Damage {
                        scale,
                        ratio,
                        flat,
                        hits: 1,
                        can_crit,
                        target,
                    },
                );
                self.damage_element = saved;
                self.damage_attack_type = saved_attack_type;
                result?;
            }
            Effect::AttackFinish => {
                self.dispatch(BattleEvent::AfterAttack, Some(actor), Some(action))?;
            }
            Effect::RetargetAttackRandom {
                ref effects,
                ref empty_effects,
            } => {
                if self.attack_owner != Some(actor) {
                    return Err("attack target list does not belong to callback owner".into());
                }
                let choices: Vec<_> = self
                    .attack_targets
                    .iter()
                    .copied()
                    .filter(|&i| self.units[i].alive())
                    .collect();
                if choices.is_empty() {
                    for child in empty_effects {
                        self.apply_effect(actor, action, child)?;
                    }
                } else {
                    let selected =
                        choices[(uniform(&mut self.targets) * choices.len() as f64) as usize];
                    let saved = self.context_target.replace(selected);
                    let result = effects
                        .iter()
                        .try_for_each(|child| self.apply_effect(actor, action, child));
                    self.context_target = saved;
                    result?;
                }
            }
            Effect::AddModifier {
                ref id,
                ref stacks,
                target,
            } => {
                let definition = self
                    .input
                    .scenario()
                    .mechanics
                    .modifier_definitions
                    .get(id)
                    .ok_or_else(|| format!("unknown modifier definition {id}"))?
                    .clone();
                let count = self.evaluate(actor, stacks)?;
                if count < 1.0 || count > 10_000.0 || count.fract() != 0.0 {
                    return Err("modifier stacks must be an integer in 1..10000".into());
                }
                let values = definition
                    .properties
                    .iter()
                    .map(|p| self.evaluate(actor, &p.value).map(|v| (p.property, v)))
                    .collect::<Result<Vec<_>, _>>()?;
                let source = self.units[actor].spec.id.clone();
                for recipient in self.targets(actor, target)? {
                    let weakness_before = self.weakness_state(recipient);
                    let count = self.units[recipient].modifiers.apply(
                        id,
                        &source,
                        count as u32,
                        &definition,
                        values.clone(),
                    );
                    self.emit(
                        "modifier_applied",
                        actor,
                        Some(recipient),
                        Some(id),
                        Some(f64::from(count)),
                        None,
                    );
                    self.record_weakness_change(recipient, weakness_before, Some(id));
                }
            }
            Effect::RemoveModifier {
                ref id,
                from_any_source,
                target,
            } => {
                let source = self.units[actor].spec.id.clone();
                for recipient in self.targets(actor, target)? {
                    let weakness_before = self.weakness_state(recipient);
                    let removed = self.units[recipient]
                        .modifiers
                        .remove(id, if from_any_source { None } else { Some(&source) });
                    for _ in removed {
                        self.emit(
                            "modifier_removed",
                            actor,
                            Some(recipient),
                            Some(id),
                            None,
                            None,
                        );
                    }
                    self.record_weakness_change(recipient, weakness_before, Some(id));
                }
            }
            Effect::Conditional {
                ref condition,
                ref then_effects,
                ref else_effects,
            } => {
                let branch = if self.condition(actor, condition)? {
                    then_effects
                } else {
                    else_effects
                };
                for child in branch {
                    self.apply_effect(actor, action, child)?;
                }
            }
            Effect::Repeat {
                ref count,
                ref effects,
            } => {
                // Evaluate once at loop entry; the body may mutate the source value.
                let count = self.evaluate(actor, count)?;
                if count < 0.0 || count > 10_000.0 || count.fract() != 0.0 {
                    return Err("repeat count must be an integer in 0..10000".into());
                }
                for _ in 0..count as u32 {
                    for child in effects {
                        self.apply_effect(actor, action, child)?;
                    }
                }
            }
            Effect::SetDynamicValue {
                ref key,
                ref value,
                target,
            } => {
                for recipient in self.targets(actor, target)? {
                    let amount = self.evaluate(recipient, value)?;
                    self.units[recipient]
                        .dynamic_values
                        .insert(key.clone(), amount);
                    self.emit(
                        "dynamic_value",
                        actor,
                        Some(recipient),
                        Some(key),
                        Some(amount),
                        None,
                    );
                }
            }
            Effect::DynamicEnergy { ref amount, target } => {
                let value = self.evaluate(actor, amount)?;
                self.apply_effect(
                    actor,
                    action,
                    &Effect::Energy {
                        amount: value,
                        target,
                    },
                )?;
            }
            Effect::Damage {
                scale,
                ratio,
                flat,
                hits,
                can_crit,
                target,
            } => {
                let base = finite(self.units[actor].scale(scale) * ratio + flat, "base damage")?;
                for _ in 0..hits {
                    for victim in self.targets(actor, target)? {
                        // Earlier recipients can dispatch immediate damage. A
                        // cached AoE target list is not proof this victim lives.
                        if !self.units[victim].alive() {
                            continue;
                        }
                        if self.attack_owner == Some(actor)
                            && !self.attack_targets.contains(&victim)
                        {
                            self.attack_targets.push(victim);
                        }
                        // A prior hit's callbacks may have changed active properties.
                        let mut attacker = self.units[actor].spec.stats.clone();
                        attacker.crit_damage = finite(
                            attacker.crit_damage
                                + self.units[actor].modifiers.delta(Property::CritDamage)?,
                            "modified crit damage",
                        )?;
                        attacker.damage_bonus = finite(
                            attacker.damage_bonus
                                + self.units[actor].modifiers.delta(Property::DamageBonus)?,
                            "modified damage bonus",
                        )?;
                        let defender = &self.units[victim].spec.stats;
                        let resistance = if let Some(element) = self.damage_element {
                            let profiles = &self.input.scenario().mechanics.element_profiles;
                            let source = profiles
                                .get(&self.units[actor].spec.id)
                                .ok_or("missing attack element panel")?;
                            let target = profiles
                                .get(&self.units[victim].spec.id)
                                .ok_or("missing defense element panel")?;
                            attacker.damage_bonus = finite(
                                attacker.damage_bonus
                                    + source
                                        .damage_bonus
                                        .get(&element)
                                        .ok_or("missing element damage bonus")?,
                                "element damage bonus",
                            )?;
                            *target
                                .resistance
                                .get(&element)
                                .ok_or("missing element resistance")?
                        } else {
                            defender.resistance
                        };
                        let critical =
                            can_crit && uniform(&mut self.criticals) < attacker.crit_rate;
                        let amount = math::damage(
                            &math::DamageInput {
                                base_damage: base,
                                attacker_level: attacker.level,
                                defense: defender.defense,
                                defense_reduction: attacker.defense_reduction,
                                defense_ignore: attacker.defense_ignore,
                                resistance,
                                resistance_penetration: attacker.resistance_penetration,
                                damage_bonus: attacker.damage_bonus,
                                vulnerability: defender.vulnerability,
                                toughness_broken: defender.toughness_broken,
                                damage_reduction: defender.damage_reduction,
                                independent_multiplier: attacker.independent_multiplier,
                                crit_damage: attacker.crit_damage,
                            },
                            critical,
                        )?
                        .amount
                            * if self.units[actor].ally {
                                self.input.scenario().mechanics.ally_damage_factor
                            } else {
                                1.0
                            };
                        finite(amount, "mode damage")?;
                        let absorbed = amount.min(self.units[victim].shield);
                        self.units[victim].shield -= absorbed;
                        let hp_loss = (amount - absorbed).min(self.units[victim].hp);
                        self.units[victim].hp -= hp_loss;
                        let died_from_this_hit = self.units[victim].hp == 0.0;
                        self.emit(
                            "damage",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(amount),
                            Some(critical),
                        );
                        self.emit(
                            "shield_absorbed",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(absorbed),
                            None,
                        );
                        self.emit(
                            "hp_loss",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(hp_loss),
                            None,
                        );
                        // The active element belongs to this hit, not nested hits
                        // dispatched by listeners. Such hits select their own panel.
                        let active_element = self.damage_element.take();
                        let active_attack_type = self.damage_attack_type.take();
                        let prior_event_attack_type = self
                            .event_attack_type
                            .replace(active_attack_type.unwrap_or(AttackType::Direct));
                        let event_result: Result<(), String> = (|| {
                            self.dispatch(BattleEvent::Hit, Some(actor), Some(action))?;
                            if died_from_this_hit {
                                self.resolve_death(victim, action)?;
                            }
                            Ok(())
                        })();
                        self.damage_element = active_element;
                        self.damage_attack_type = active_attack_type;
                        self.event_attack_type = prior_event_attack_type;
                        event_result?;
                    }
                }
            }
            Effect::Heal {
                scale,
                ratio,
                flat,
                target,
            } => {
                let base = finite(
                    self.units[actor].scale(scale) * ratio + flat,
                    "base healing",
                )?;
                for recipient in self.targets(actor, target)? {
                    let raw = math::heal(
                        base,
                        self.units[actor].spec.stats.healing_bonus,
                        self.units[recipient].spec.stats.incoming_healing_bonus,
                    )?;
                    let actual =
                        raw.min(self.units[recipient].spec.stats.max_hp - self.units[recipient].hp);
                    self.units[recipient].hp += actual;
                    self.emit(
                        "heal",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(actual),
                        None,
                    );
                }
            }
            Effect::Shield {
                scale,
                ratio,
                flat,
                max_total,
                duration_turns,
                target,
            } => {
                let base = finite(self.units[actor].scale(scale) * ratio + flat, "base shield")?;
                let raw = math::shield(base, self.units[actor].spec.stats.shield_bonus)?;
                for recipient in self.targets(actor, target)? {
                    let before = self.units[recipient].shield;
                    // A lower cap never removes a pre-existing stronger shield.
                    let actual = raw.min((max_total - before).max(0.0));
                    self.units[recipient].shield += actual;
                    self.units[recipient].shield_turns =
                        self.units[recipient].shield_turns.max(duration_turns);
                    self.emit(
                        "shield",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(actual),
                        None,
                    );
                }
            }
            Effect::Energy { amount, target } => {
                for recipient in self.targets(actor, target)? {
                    let before = self.units[recipient].energy;
                    self.units[recipient].energy =
                        (before + amount).clamp(0.0, self.units[recipient].spec.max_energy);
                    self.emit(
                        "energy",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(self.units[recipient].energy - before),
                        None,
                    );
                }
            }
            Effect::Advance { fraction, target } | Effect::AdvanceNext { fraction, target } => {
                for recipient in self.targets(actor, target)? {
                    if matches!(effect, Effect::AdvanceNext { .. }) {
                        self.units[recipient].next_shift += DISTANCE * fraction;
                    } else if self.turn != Some(recipient) {
                        self.units[recipient].remaining =
                            (self.units[recipient].remaining - DISTANCE * fraction).max(0.0);
                    }
                    self.emit(
                        "advance",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(self.units[recipient].remaining),
                        None,
                    );
                }
            }
            Effect::Speed { value, target } => {
                for recipient in self.targets(actor, target)? {
                    self.units[recipient].spec.stats.speed = value;
                    self.emit(
                        "speed",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(value),
                        None,
                    );
                }
            }
            Effect::Toughness { amount, target } => {
                for recipient in self.targets(actor, target)? {
                    if !self.units[recipient].alive() {
                        continue;
                    }
                    let before = self.units[recipient].toughness;
                    self.units[recipient].toughness = (before - amount).max(0.0);
                    if before > 0.0 && self.units[recipient].toughness == 0.0 {
                        self.units[recipient].spec.stats.toughness_broken = true;
                        self.emit("break", actor, Some(recipient), Some(action), None, None);
                        self.dispatch(BattleEvent::Break, Some(recipient), Some(action))?;
                    }
                }
            }
            Effect::Request {
                kind,
                ref action,
                target,
            } => {
                for recipient in self.targets(actor, target)? {
                    let selected = action
                        .as_ref()
                        .map(|id| {
                            self.units[recipient]
                                .spec
                                .actions
                                .iter()
                                .position(|a| &a.id == id)
                                .ok_or_else(|| format!("unknown requested action {id}"))
                        })
                        .transpose()?;
                    self.enqueue(Pending {
                        actor: recipient,
                        kind,
                        action: selected,
                    })?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, actor: usize, action_index: usize, prepaid: bool) -> Result<(), String> {
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
        for effect in &action.effects {
            self.apply_effect(actor, &action.id, effect)?;
        }
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
    fn finish(
        self,
        seed: u64,
        outcome: Outcome,
        termination: Termination,
        waves_cleared: usize,
    ) -> BattleResult {
        let weaknesses: Vec<_> = (0..self.units.len())
            .map(|i| self.weakness_state(i))
            .collect();
        BattleResult {
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
                .map(|(u, weaknesses)| UnitResult {
                    weaknesses,
                    id: u.spec.id,
                    ally: u.ally,
                    hp: u.hp,
                    shield: u.shield,
                    energy: u.energy,
                    alive: u.hp > 0.0,
                    revivals_left: u.revivals_left,
                    remaining_distance: u.remaining,
                    speed: u.spec.stats.speed,
                    toughness: u.toughness,
                    dynamic_values: u.dynamic_values,
                    modifiers: u.modifiers.states(),
                })
                .collect(),
            events: self.events,
            trace_truncated: self.trace_truncated,
            run_hp_loss: None,
            validation_scope:
                "explicit_battle_v2_operations; imported_config_coverage_is_recorded_separately"
                    .into(),
        }
    }
}

/// Run a single scenario without cancellation; the input remains unchanged.
pub fn simulate(input: &ValidatedScenario, seed: u64, trace: bool) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, trace, &mut || false)
}
/// Check cancellation before each action, including immediate ultimate chains.
/// Checks occur at action boundaries before selecting an action or advancing AV,
/// never partway through an action's effects. A callback returning `true` yields
/// `Ok(BattleResult)` with [`Outcome::Incomplete`] and [`Termination::Cancelled`].
/// Settled terminal elimination and already exhausted AV take precedence and
/// return their results without polling cancellation at that boundary.
pub fn simulate_cancellable(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    simulate_with_policy(input, seed, trace, cancel, &mut Automatic)
}

/// Execute through legal operations; policy cannot mutate battle state.
pub fn simulate_with_policy(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl Policy,
) -> Result<BattleResult, String> {
    let mut state = State::new(input, seed, trace);
    state.dispatch(BattleEvent::BattleStart, None, None)?;
    state.dispatch(BattleEvent::WaveStart, None, None)?;
    loop {
        let allies = state.units.iter().any(|u| u.ally && u.alive());
        let enemies = state.units.iter().any(|u| !u.ally && u.alive());
        let cleared = state.wave + usize::from(!enemies);
        if !allies && !enemies {
            return Ok(state.finish(seed, Outcome::Draw, Termination::BothDefeated, cleared));
        }
        if !allies {
            return Ok(state.finish(seed, Outcome::Defeat, Termination::AlliesDefeated, cleared));
        }
        if !enemies {
            if state.wave + 1 == input.scenario().waves.len() {
                return Ok(state.finish(
                    seed,
                    Outcome::Victory,
                    Termination::EnemiesDefeated,
                    cleared,
                ));
            }
            state.wave += 1;
            for u in &input.scenario().waves[state.wave] {
                let mut unit = CombatUnit::new(u, false, state.timeline.now());
                unit.toughness = *input
                    .scenario()
                    .mechanics
                    .toughness
                    .get(&u.id)
                    .unwrap_or(&0.0);
                if let Some(values) = input.scenario().mechanics.dynamic_values.get(&u.id) {
                    unit.dynamic_values = values.clone();
                }
                state.units.push(unit);
            }
            state.dispatch(BattleEvent::WaveStart, None, None)?;
            continue;
        }
        // Deadline is settled before safety/cancellation, after complete actions.
        if state.exhausted {
            return Ok(state.finish(
                seed,
                Outcome::Defeat,
                Termination::AvBudgetExhausted,
                cleared,
            ));
        }
        let zero_time_due = input.scenario().mechanics.unit_before_cycle
            && (state.turn.is_some()
                || !state.pending.is_empty()
                || state.units.iter().any(|u| u.alive() && u.remaining == 0.0));
        if state.timeline.remaining == 0.0 && !zero_time_due {
            state.emit(
                "cycle_end",
                0,
                None,
                None,
                Some((state.timeline.cycle + 1) as f64),
                None,
            );
            state.dispatch(BattleEvent::CycleEnd, None, None)?;
            if !state.timeline.next_cycle() {
                state.exhausted = true;
            }
            continue;
        }
        if cancel() {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::Cancelled, cleared));
        }
        if state.actions >= input.scenario().max_actions {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::ActionLimit, cleared));
        }
        if let Some(op) = state.decide(DecisionPoint::Insert, None, policy)? {
            state.enqueue(Pending {
                actor: op.actor,
                kind: RequestKind::Ultimate,
                action: Some(op.action),
            })?;
        }
        if let Some(request) = state.pending.pop_front() {
            let actor = request.actor;
            if !state.units[actor].alive() {
                state.emit("request_invalid", actor, None, None, None, None);
                continue;
            }
            let selected = if request.kind == RequestKind::ExtraTurn {
                state.emit("extra_turn_start", actor, None, None, None, None);
                state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                if !state.units[actor].alive() {
                    continue;
                }
                state
                    .decide(DecisionPoint::ExtraTurn, Some(actor), policy)?
                    .map(|op| op.action)
            } else {
                request.action
            };
            let action = selected.ok_or("turn policy declined all legal actions")?;
            if !state.affordable(actor, &state.units[actor].spec.actions[action]) {
                state.emit("request_invalid", actor, None, None, None, None);
                continue;
            }
            if request.kind == RequestKind::Ultimate {
                let skill = state.units[actor].spec.actions[action].clone();
                state.pay(actor, &skill);
                state.emit(
                    "ultimate_turn_start",
                    actor,
                    None,
                    None,
                    Some(state.units[actor].energy),
                    None,
                );
                state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                if !state.units[actor].alive() {
                    continue;
                }
            }
            state.act(actor, action, request.kind == RequestKind::Ultimate)?;
            if request.kind != RequestKind::FollowUp {
                state.dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
                state.emit("extra_turn_end", actor, None, None, None, None);
            }
            continue;
        }
        if let Some(actor) = state.turn {
            state.dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
            state.units[actor].remaining = (DISTANCE - state.units[actor].next_shift).max(0.0);
            state.units[actor].next_shift = 0.0;
            state.turn = None;
            state.emit(
                "normal_turn_end",
                actor,
                None,
                None,
                Some(state.units[actor].remaining),
                None,
            );
            continue;
        }
        let (actor, wait) = scheduler::next_normal(
            state
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| u.alive())
                .map(|(i, u)| (i, u.remaining, u.spec.stats.speed)),
        )
        .ok_or("empty axis")?;
        let cycle_first = state.timeline.remaining < wait
            || (state.timeline.remaining == wait && !input.scenario().mechanics.unit_before_cycle);
        let delta = wait.min(state.timeline.remaining);
        state.timeline.advance(delta);
        for u in &mut state.units {
            if u.alive() {
                u.remaining = (u.remaining - delta * u.spec.stats.speed).max(0.0);
            }
        }
        if cycle_first {
            continue;
        }
        state.turn = Some(actor);
        state.emit(
            "normal_turn_start",
            actor,
            None,
            None,
            Some(state.units[actor].remaining),
            None,
        );
        if state.units[actor].shield_turns > 0 {
            state.units[actor].shield_turns -= 1;
            if state.units[actor].shield_turns == 0 {
                let expired = state.units[actor].shield;
                state.units[actor].shield = 0.0;
                state.emit(
                    "shield_expired",
                    actor,
                    Some(actor),
                    None,
                    Some(expired),
                    None,
                );
            }
        }
        state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
        if !state.units[actor].alive() {
            continue;
        }
        let op = state
            .decide(DecisionPoint::NormalTurn, Some(actor), policy)?
            .ok_or("turn policy declined all legal actions")?;
        state.act(actor, op.action, false)?;
    }
}
