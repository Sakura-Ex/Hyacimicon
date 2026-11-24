//! 按输入顺序执行效果树并限制递归和工作量。
use super::*;

impl State<'_> {
    /// 顺序执行每个子效果，保留单效果入口的计数和递归限制。
    pub(super) fn apply_effects(
        &mut self,
        actor: usize,
        action: &str,
        effects: &[Effect],
    ) -> Result<(), String> {
        effects
            .iter()
            .try_for_each(|effect| self.apply_effect(actor, action, effect))
    }

    /// 执行一个效果节点并维护当前递归深度和执行量。
    pub(super) fn apply_effect(
        &mut self,
        actor: usize,
        action: &str,
        effect: &Effect,
    ) -> Result<(), String> {
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

    /// 按效果分支修改资源、属性、变量或排队请求。
    pub(super) fn apply_effect_inner(
        &mut self,
        actor: usize,
        action: &str,
        effect: &Effect,
    ) -> Result<(), String> {
        match *effect {
            Effect::WithDynamicScope { scope, ref effects } => {
                self.dynamic_scopes.push(actor, scope);
                trace!(
                    self,
                    &format!("{}_scope_enter", scope.name()),
                    actor,
                    Some(actor),
                    Some(action),
                    None,
                    None,
                );
                let result = self.apply_effects(actor, action, effects);
                self.dynamic_scopes.pop();
                trace!(
                    self,
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
                trace!(
                    self,
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
                        #[cfg(feature = "diagnostics")]
                        if self.recorded_random.is_some() {
                            return Err(format!("recorded replay does not yet support random elements in {action:?}"));
                        }
                        Element::ALL[(uniform(&mut self.elements) * 7.0) as usize]
                    }
                };
                trace!(
                    self,
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
                    self.apply_effects(actor, action, empty_effects)?;
                } else {
                    let selected =
                        self.draw_target(actor, action, &choices, rng::TargetDraw::HitRetarget)?;
                    let saved = self.context_target.replace(selected);
                    let result = self.apply_effects(actor, action, effects);
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
                for recipient in self.targets(actor, action, target)? {
                    #[cfg(feature = "diagnostics")]
                    let weakness_before = self.weakness_state(recipient);
                    let _count = self.units[recipient].modifiers.apply(
                        id,
                        &source,
                        count as u32,
                        &definition,
                        values.clone(),
                    );
                    trace!(
                        self,
                        "modifier_applied",
                        actor,
                        Some(recipient),
                        Some(id),
                        Some(f64::from(_count)),
                        None,
                    );
                    #[cfg(feature = "diagnostics")]
                    self.record_weakness_change(recipient, weakness_before, Some(id));
                }
            }
            Effect::RemoveModifier {
                ref id,
                from_any_source,
                target,
            } => {
                let source = self.units[actor].spec.id.clone();
                for recipient in self.targets(actor, action, target)? {
                    #[cfg(feature = "diagnostics")]
                    let weakness_before = self.weakness_state(recipient);
                    let removed = self.units[recipient]
                        .modifiers
                        .remove(id, if from_any_source { None } else { Some(&source) });
                    for _ in removed {
                        trace!(
                            self,
                            "modifier_removed",
                            actor,
                            Some(recipient),
                            Some(id),
                            None,
                            None,
                        );
                    }
                    #[cfg(feature = "diagnostics")]
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
                self.apply_effects(actor, action, branch)?;
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
                    self.apply_effects(actor, action, effects)?;
                }
            }
            Effect::SetDynamicValue {
                ref key,
                ref value,
                target,
            } => {
                for recipient in self.targets(actor, action, target)? {
                    let amount = self.evaluate(recipient, value)?;
                    self.units[recipient]
                        .dynamic_values
                        .insert(key.clone(), amount);
                    trace!(
                        self,
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
                self.damage_hits(actor, action, base, hits, can_crit, target)?;
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
                for recipient in self.targets(actor, action, target)? {
                    let raw = math::heal(
                        base,
                        self.units[actor].spec.stats.healing_bonus,
                        self.units[recipient].spec.stats.incoming_healing_bonus,
                    )?;
                    let actual =
                        raw.min(self.units[recipient].spec.stats.max_hp - self.units[recipient].hp);
                    self.units[recipient].hp += actual;
                    trace!(
                        self,
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
                for recipient in self.targets(actor, action, target)? {
                    let before = self.units[recipient].shield;
                    // A lower cap never removes a pre-existing stronger shield.
                    let actual = raw.min((max_total - before).max(0.0));
                    self.units[recipient].shield += actual;
                    self.units[recipient].shield_turns =
                        self.units[recipient].shield_turns.max(duration_turns);
                    trace!(
                        self,
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
                for recipient in self.targets(actor, action, target)? {
                    let before = self.units[recipient].energy;
                    self.units[recipient].energy =
                        (before + amount).clamp(0.0, self.units[recipient].spec.max_energy);
                    trace!(
                        self,
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
                for recipient in self.targets(actor, action, target)? {
                    if matches!(effect, Effect::AdvanceNext { .. }) {
                        self.units[recipient].next_shift += DISTANCE * fraction;
                    } else if self.turn != Some(recipient) {
                        self.units[recipient].remaining =
                            (self.units[recipient].remaining - DISTANCE * fraction).max(0.0);
                    }
                    trace!(
                        self,
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
                for recipient in self.targets(actor, action, target)? {
                    let unit = &mut self.units[recipient];
                    if unit.summon.is_some() && (!value.is_finite() || value <= 0.0) {
                        return Err(format!(
                            "traditional summon {} needs finite positive speed",
                            unit.spec.id
                        ));
                    }
                    unit.spec.stats.speed = value;
                    trace!(
                        self,
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
                self.reduce_toughness(actor, action, amount, target, None)?;
            }
            Effect::ImaginaryToughness {
                amount,
                break_damage,
                target,
            } => {
                if !amount.is_finite() || amount < 0.0 {
                    return Err("imaginary toughness damage must be finite and non-negative".into());
                }
                self.reduce_toughness(actor, action, amount, target, Some(break_damage))?;
            }
            Effect::Request {
                kind,
                action: ref requested_action,
                target,
            } => {
                for recipient in self.targets(actor, action, target)? {
                    let selected = requested_action
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
                        primary_enemy: None,
                    })?;
                }
            }
        }
        Ok(())
    }

    /// 扣除韧性，只有当活着的目标韧性从正数降到零时产生击破。
    fn reduce_toughness(
        &mut self,
        actor: usize,
        action: &str,
        amount: f64,
        target: Target,
        imaginary: Option<ImaginaryBreakDamage>,
    ) -> Result<(), String> {
        for recipient in self.targets(actor, action, target)? {
            if !self.units[recipient].alive() {
                continue;
            }
            let before = self.units[recipient].toughness;
            let break_input = if let Some(profile) = imaginary {
                let id = &self.units[recipient].spec.id;
                let max = self.input.mechanics.max_toughness.get(id).copied();
                if !before.is_finite() || before < 0.0 {
                    return Err(format!(
                        "imaginary break target {id} current toughness must be finite and non-negative"
                    ));
                }
                if let Some(max) = max {
                    if !max.is_finite() || max <= 0.0 {
                        return Err(format!(
                            "imaginary break target {id} max_toughness must be finite and positive"
                        ));
                    }
                    if before > max {
                        return Err(format!(
                            "imaginary break target {id} current toughness {before} exceeds max_toughness {max}"
                        ));
                    }
                }
                Some((profile, max))
            } else {
                None
            };
            let after = (before - amount).max(0.0);
            if !(before > 0.0 && after == 0.0) {
                self.units[recipient].toughness = after;
                continue;
            }

            let damage = break_input
                .map(|(profile, max)| {
                    let id = &self.units[recipient].spec.id;
                    let max = max.ok_or_else(|| {
                        format!("missing max_toughness for imaginary break target {id}")
                    })?;
                    self.imaginary_break_amount(actor, recipient, max, profile)
                })
                .transpose()?;
            let revivals_left = self.units[recipient].revivals_left;
            self.units[recipient].toughness = after;
            self.units[recipient].spec.stats.toughness_broken = true;
            trace!(
                self,
                "break",
                actor,
                Some(recipient),
                Some(action),
                None,
                None
            );
            self.dispatch(BattleEvent::Break, Some(recipient), Some(action))?;
            // Break listeners can kill the target or end its current life by reviving it.
            if let Some(amount) = damage.filter(|_| {
                self.units[recipient].alive()
                    && self.units[recipient].revivals_left == revivals_left
            }) {
                if self.apply_damage("break_damage", actor, recipient, action, amount, false) {
                    // A break triggered by a hit does not inherit that hit's category.
                    let prior_event_attack_type = self.event_attack_type.take();
                    let result = self.resolve_death(recipient, action);
                    self.event_attack_type = prior_event_attack_type;
                    result?;
                }
            }
        }
        Ok(())
    }
}

/// 返回有限计算结果，遇到 NaN 或无穷值时附带字段名报错。
pub(super) fn finite(value: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite {name}"))
    }
}
