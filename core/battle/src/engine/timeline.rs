//! 按剩余行动距离与速度选择下一行动者，维护轮次 AV 预算。
//! 单位在 TurnEnd 时重排行动距离。
pub const DISTANCE: f64 = 10000.0;
pub struct Timeline {
    pub now: f64,
    pub cycle: usize,
    pub remaining: f64,
    pub penalty: f64,
    pub budgets: Vec<f64>,
}
impl Timeline {
    pub fn new(budgets: Vec<f64>) -> Self {
        Self {
            now: 0.0,
            cycle: 0,
            remaining: budgets[0],
            penalty: 0.0,
            budgets,
        }
    }
    pub fn now(&self) -> f64 {
        self.now
    }
    pub fn penalty(&self) -> f64 {
        self.penalty
    }
    pub fn remaining_total(&self) -> f64 {
        self.remaining + self.budgets.iter().skip(self.cycle + 1).sum::<f64>()
    }
    pub fn penalize(&mut self, amount: f64) -> f64 {
        let applied = amount.min(self.remaining);
        self.remaining -= applied;
        self.penalty += applied;
        applied
    }
    pub fn advance(&mut self, delta: f64) {
        self.now += delta;
        self.remaining = (self.remaining - delta).max(0.0);
    }
    pub fn next_cycle(&mut self) -> bool {
        if self.cycle + 1 >= self.budgets.len() {
            false
        } else {
            self.cycle += 1;
            self.remaining = self.budgets[self.cycle];
            true
        }
    }
}
pub fn next_normal(items: impl Iterator<Item = (usize, f64, f64)>) -> Option<(usize, f64)> {
    items
        .map(|(i, d, s)| (i, d / s))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}
