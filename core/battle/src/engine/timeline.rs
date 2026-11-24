//! 按剩余行动距离与速度选择下一行动者，维护轮次 AV 预算。
//! 单位在 TurnEnd 时重排行动距离。
use super::finite;

/// 普通行动的标准距离，等待 AV 由剩余距离除以速度计算。
pub const DISTANCE: f64 = 10000.0;

/// 实际 AV 时钟及各倒计时轮次的剩余额度。
pub struct Timeline {
    /// 实际推进的 AV 时钟。
    pub now: f64,
    /// 当前倒计时轮次的内部索引，从零开始。
    pub cycle: usize,
    /// 当前 Cycle 剩余 AV 额度。
    pub remaining: f64,
    /// 累计实际扣除的当前轮罚时。
    pub penalty: f64,
    /// 各倒计时轮次的初始额度。
    budgets: Vec<f64>,
    /// Sum of the later allowances in battle order, validated on cycle entry.
    later_allowances: f64,
}
impl Timeline {
    /// Initialize the clock from a finite total and optional per-cycle allowances.
    pub fn new(max_av: f64, cycles: &[f64]) -> Result<Self, String> {
        if !max_av.is_finite() || max_av < 0.0 {
            return Err("max_av must be finite and non-negative".into());
        }
        let allowances = if cycles.is_empty() {
            std::slice::from_ref(&max_av)
        } else {
            cycles
        };
        if allowances.iter().any(|av| !av.is_finite() || *av < 0.0) {
            return Err("mechanics.cycles must contain finite, non-negative AV".into());
        }
        let remaining = allowances[0];
        let (later_allowances, total) = checked_cycle_total(remaining, &allowances[1..])?;
        // Allow rounding when adding finite non-negative floating-point allowances.
        let tolerance = f64::EPSILON * max_av.max(total) * allowances.len() as f64;
        if (total - max_av).abs() > tolerance {
            return Err(format!(
                "mechanics.cycles sum to {total} AV, expected max_av {max_av}"
            ));
        }
        Ok(Self {
            now: 0.0,
            cycle: 0,
            remaining,
            penalty: 0.0,
            budgets: allowances.to_vec(),
            later_allowances,
        })
    }
    /// 返回已实际推进的 AV 时钟。
    pub fn now(&self) -> f64 {
        self.now
    }
    /// 返回累计实际扣除的复活罚时。
    pub fn penalty(&self) -> f64 {
        self.penalty
    }
    /// 返回当前轮余额与后续轮初始额度之和。
    pub fn remaining_total(&self) -> f64 {
        self.remaining + self.later_allowances
    }
    /// 最多扣光当前轮额度，累计并返回实际扣除量。
    pub fn penalize(&mut self, amount: f64) -> Result<f64, String> {
        if !amount.is_finite() || amount < 0.0 {
            return Err("revival AV penalty must be finite and non-negative".into());
        }
        let applied = amount.min(self.remaining);
        self.penalty = finite(self.penalty + applied, "cumulative revival AV penalty")?;
        self.remaining -= applied;
        Ok(applied)
    }
    /// 推进实际 AV 并扣减当前轮额度。
    pub fn advance(&mut self, delta: f64) -> Result<(), String> {
        if delta < 0.0 {
            return Err("AV advance must be non-negative".into());
        }
        self.now = finite(self.now + delta, "elapsed AV")?;
        self.remaining = (self.remaining - delta).max(0.0);
        Ok(())
    }
    /// 存在下一轮时装载其额度并返回 true。
    pub fn next_cycle(&mut self) -> Result<bool, String> {
        let next = self.cycle + 1;
        let Some((&remaining, later)) = self.budgets[next..].split_first() else {
            return Ok(false);
        };
        let later_allowances = if remaining == 0.0 {
            // The previous tail starts with zero, so its sum already covers this tail.
            self.later_allowances
        } else {
            checked_cycle_total(remaining, later)?.0
        };
        self.cycle = next;
        self.remaining = remaining;
        self.later_allowances = later_allowances;
        Ok(true)
    }
}

/// Keep the established reporting order and validate the full current allowance.
fn checked_cycle_total(remaining: f64, later: &[f64]) -> Result<(f64, f64), String> {
    let later_allowances = later.iter().sum::<f64>();
    let total = finite(remaining + later_allowances, "mechanics.cycles total AV")?;
    Ok((later_allowances, total))
}

/// 按距离除以速度选择最近的单位，同刻按索引稳定排序。
pub fn next_normal(items: impl Iterator<Item = (usize, f64, f64)>) -> Option<(usize, f64)> {
    items
        .map(|(i, d, s)| (i, d / s))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}
