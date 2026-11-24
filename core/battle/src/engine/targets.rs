//! 按声明的选择器查找当前存活目标。
use super::*;

impl State<'_> {
    /// Only living regular opponents may be selected as primary enemies.
    pub(super) fn is_primary_enemy(&self, actor: usize, index: usize) -> bool {
        let unit = &self.units[index];
        unit.regular_alive() && unit.ally != self.units[actor].ally
    }

    /// Return selectable runtime instances for an already validated operation.
    pub(super) fn primary_enemy_candidates(
        &self,
        actor: usize,
        action: usize,
    ) -> Result<impl Iterator<Item = (usize, &str)> + '_, String> {
        let action = &self.units[actor].spec.actions[action];
        if !has_bound_enemy_hit(&action.effects, 0) {
            return Err("action does not accept a primary enemy target".into());
        }
        Ok(self
            .units
            .iter()
            .enumerate()
            .filter_map(move |(index, unit)| {
                self.is_primary_enemy(actor, index)
                    .then_some((index, unit.spec.id.as_str()))
            }))
    }

    /// 按 target 取得当前存活目标，随机选择消耗目标随机流。
    pub(super) fn targets(
        &mut self,
        actor: usize,
        action: &str,
        target: Target,
    ) -> Result<Vec<usize>, String> {
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
            .filter(|(_, u)| u.regular_alive() && (u.ally == side) == allies)
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
            Target::EnemyRandomUniform => Ok(vec![self.draw_target(
                actor,
                action,
                &candidates,
                rng::TargetDraw::UniformEnemy,
            )?]),
            Target::EnemyRandomTaunt => Ok(vec![self.draw_target(
                actor,
                action,
                &candidates,
                rng::TargetDraw::TauntEnemy,
            )?]),
            Target::AbilityTarget => unreachable!("bound target handled above"),
            Target::ContextTarget => unreachable!("context target handled above"),
            Target::SelfUnit => unreachable!("self target handled before candidate selection"),
        }
    }
    // Called only by the hit that changed this unit's HP from positive to zero.
    // Do not infer a new death from HP observed after a nested listener returns.
}

/// A direct hit with a bound target opts in. Effects exclusively behind a
/// condition or retarget scope do not advertise a selectable primary enemy.
fn has_bound_enemy_hit(effects: &[Effect], depth: usize) -> bool {
    depth < 64
        && effects.iter().any(|effect| match effect {
            Effect::Damage {
                hits: 1..,
                target: Target::AbilityTarget,
                ..
            }
            | Effect::ElementalDamage {
                target: Target::AbilityTarget,
                ..
            } => true,
            Effect::WithDynamicScope { effects, .. } => has_bound_enemy_hit(effects, depth + 1),
            _ => false,
        })
}
