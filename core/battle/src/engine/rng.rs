//! Fixed seed streams and replay-time selection against the current legal pool.
#[cfg(feature = "diagnostics")]
use super::state::CombatUnit;
use super::{finite, State};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetDraw {
    UniformEnemy,
    TauntEnemy,
    HitRetarget,
}

#[cfg(feature = "diagnostics")]
impl TargetDraw {
    fn kind(self) -> &'static str {
        match self {
            Self::UniformEnemy => "uniform_enemy",
            Self::TauntEnemy => "taunt_enemy",
            Self::HitRetarget => "hit_retarget",
        }
    }
}

/// 用种子和用途域派生一条独立 ChaCha8 随机流。
pub(super) fn stream(seed: u64, domain: &[u8]) -> ChaCha8Rng {
    let mut hash = Sha256::new();
    // 保留随机流派生域，保证既有种子的回放结果一致。
    hash.update(b"hcwa-battle-core/rng/v1\0");
    hash.update(domain);
    hash.update(seed.to_le_bytes());
    ChaCha8Rng::from_seed(hash.finalize().into())
}
/// 消耗随机流生成零到一之间、包含零的均匀值。
pub(super) fn uniform(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}

/// A record's name must identify the executing or struck instance at the draw.
#[cfg(feature = "diagnostics")]
fn recorded_unit_id<'a>(
    units: &'a [CombatUnit],
    index: usize,
    role: &str,
) -> Result<&'a str, String> {
    let id = units[index].spec.id.as_str();
    if units
        .iter()
        .enumerate()
        .any(|(other, unit)| other != index && unit.alive() && unit.spec.id == id)
    {
        return Err(format!("ambiguous recorded {role} {id:?}"));
    }
    Ok(id)
}

impl State<'_> {
    pub(super) fn roll_critical(
        &mut self,
        _actor: usize,
        _action: &str,
        _victim: usize,
        can_crit: bool,
        rate: f64,
    ) -> Result<bool, String> {
        if !can_crit {
            return Ok(false);
        }
        #[cfg(feature = "diagnostics")]
        if let Some(recorded) = &mut self.recorded_random {
            let rate = finite(rate, "recorded critical rate")?;
            if rate <= 0.0 || rate >= 1.0 {
                return Ok(rate >= 1.0);
            }
            let actor = recorded_unit_id(&self.units, _actor, "actor")?;
            let victim = recorded_unit_id(&self.units, _victim, "critical target")?;
            let result = recorded.take("critical", actor, _action, Some(victim))?;
            return Ok(matches!(
                result,
                super::recorded_rng::RecordedResult::Critical { critical: true, .. }
            ));
        }
        Ok(uniform(&mut self.criticals) < rate)
    }

    /// Select from the current pool using either the recorded result or the seeded draw.
    pub(super) fn draw_target(
        &mut self,
        _actor: usize,
        _action: &str,
        candidates: &[usize],
        draw: TargetDraw,
    ) -> Result<usize, String> {
        if draw == TargetDraw::TauntEnemy {
            let total = finite(
                candidates
                    .iter()
                    .map(|&i| self.units[i].spec.stats.taunt)
                    .sum(),
                "taunt sum",
            )?;
            #[cfg(feature = "diagnostics")]
            if self.recorded_random.is_some() {
                if total <= 0.0
                    || candidates
                        .iter()
                        .any(|&i| self.units[i].spec.stats.taunt < 0.0)
                {
                    return Err(
                        "recorded taunt draw requires nonnegative weights and a positive total"
                            .into(),
                    );
                }
                let eligible: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|&i| self.units[i].spec.stats.taunt > 0.0)
                    .collect();
                return self.recorded_target(_actor, _action, &eligible, draw);
            }
            let mut sample = uniform(&mut self.targets) * total;
            for &i in candidates {
                let weight = self.units[i].spec.stats.taunt;
                if sample < weight {
                    return Ok(i);
                }
                sample -= weight;
            }
            // Preserve the seeded fallback at the upper floating-point edge.
            return candidates
                .last()
                .copied()
                .ok_or_else(|| "target selection unexpectedly empty".into());
        }
        #[cfg(feature = "diagnostics")]
        if self.recorded_random.is_some() {
            return self.recorded_target(_actor, _action, candidates, draw);
        }
        Ok(candidates[(uniform(&mut self.targets) * candidates.len() as f64) as usize])
    }

    #[cfg(feature = "diagnostics")]
    fn recorded_target(
        &mut self,
        actor: usize,
        action: &str,
        candidates: &[usize],
        draw: TargetDraw,
    ) -> Result<usize, String> {
        if let [only] = candidates {
            return Ok(*only);
        }
        let source = recorded_unit_id(&self.units, actor, "actor")?;
        let result = self
            .recorded_random
            .as_mut()
            .ok_or("recorded random source is missing")?
            .take(draw.kind(), source, action, None)?;
        let target = result.target();
        let mut eligible = candidates
            .iter()
            .copied()
            .filter(|&i| self.units[i].spec.id == target);
        let index = eligible.next().ok_or_else(|| {
            format!(
                "illegal {} target {target:?} from {source:?}/{action:?}: outside the current pool",
                draw.kind()
            )
        })?;
        if eligible.next().is_some() {
            return Err(format!(
                "ambiguous {} target {target:?} from {source:?}/{action:?}",
                draw.kind()
            ));
        }
        Ok(index)
    }
}
