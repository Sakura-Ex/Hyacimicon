//! 按主副词条组合、强化落点和数值档位的原始概率，在给定分数范围内抽一件成品。

use std::collections::HashMap;

use super::{
    Bounds, Candidate, EPSILON, GenerationError, PieceSearch, Recipe, Search, Sub, contains,
    extremes, rolls,
};

const FACTORIAL: [f64; 7] = [1.0, 1.0, 2.0, 6.0, 24.0, 120.0, 720.0];

#[derive(Clone, Copy)]
struct Prefix<'a> {
    recipe: &'a Recipe,
    position: usize,
    extras: u8,
    values: [f64; 4],
    score: f64,
}

impl<'a> Prefix<'a> {
    fn new(recipe: &'a Recipe, count: u8) -> Self {
        Self {
            recipe,
            position: 0,
            extras: count - 4,
            values: [0.0; 4],
            score: 0.0,
        }
    }

    // 剩余部分全部合法时，直接独立抽取每次强化和档位。
    fn finish(mut self, tiers: [f64; 3], search: &mut Search<'_>) -> Candidate {
        let mut counts = [1u8; 4];
        for _ in 0..self.extras {
            counts[self.position + search.index(4 - self.position)] += 1;
        }
        for (i, &count) in counts.iter().enumerate().skip(self.position) {
            let stat = self.recipe.stats[i];
            let value = PieceSearch::sample_rolls(tiers, stat, count, search);
            self.values[i] = value;
            self.score += contribution(tiers, stat, self.recipe.weights[i], count, value);
        }
        Candidate {
            main: self.recipe.main,
            stats: self.recipe.stats,
            weights: self.recipe.weights,
            values: self.values,
            score: self.score,
        }
    }
}

fn contribution(tiers: [f64; 3], stat: Sub, weight: f64, count: u8, value: f64) -> f64 {
    if tiers[0] == 0.0 && tiers[2] == 0.0 {
        weight * f64::from(count)
    } else {
        weight * (value / rolls(stat)[1])
    }
}

pub(super) struct TierOutcome {
    pub(super) counts: [u8; 3],
    pub(super) log_mass: f64,
}

struct ConditionalDraw {
    bounds: Bounds,
    tiers: [f64; 3],
    extremes: (usize, usize),
    outcomes: [Vec<TierOutcome>; 7],
}

pub(super) fn tier_outcomes(tiers: [f64; 3]) -> [Vec<TierOutcome>; 7] {
    let largest = tiers.into_iter().fold(0.0, f64::max);
    let total_log = largest.ln() + tiers.iter().map(|w| w / largest).sum::<f64>().ln();
    std::array::from_fn(|n| {
        let mut options = Vec::new();
        for low in 0..=n {
            for mid in 0..=n - low {
                let counts = [low as u8, mid as u8, (n - low - mid) as u8];
                if counts.iter().zip(tiers).any(|(&c, w)| c > 0 && w == 0.0) {
                    continue;
                }
                let log_mass = FACTORIAL[n].ln()
                    + counts
                        .iter()
                        .zip(tiers)
                        .filter(|(c, _)| **c != 0)
                        .map(|(&c, w)| {
                            f64::from(c) * (w.ln() - total_log) - FACTORIAL[c as usize].ln()
                        })
                        .sum::<f64>();
                options.push(TierOutcome { counts, log_mass });
            }
        }
        options
    })
}

type CompletionMass = HashMap<(usize, u8, u64), f64>;

impl ConditionalDraw {
    fn new(bounds: Bounds, tiers: [f64; 3]) -> Self {
        Self {
            bounds,
            tiers,
            extremes: extremes(tiers),
            outcomes: tier_outcomes(tiers),
        }
    }

    // 整段落入区间时，所有后续抽法都合法；整段在区间外时，可行概率为零。
    fn terminal_mass(&self, prefix: Prefix<'_>) -> Option<f64> {
        let (lo, hi) = prefix
            .recipe
            .tail_bounds(prefix.position, prefix.extras, self.extremes);
        let lo = prefix.score + lo;
        let hi = prefix.score + hi;
        if contains(self.bounds, lo) && contains(self.bounds, hi) {
            return Some(0.0);
        }
        if prefix.position == 4 || hi < self.bounds.0 - EPSILON || lo > self.bounds.1 + EPSILON {
            return Some(f64::NEG_INFINITY);
        }
        None
    }

    fn branches<'r>(&self, prefix: Prefix<'r>) -> impl Iterator<Item = (Prefix<'r>, f64)> {
        let i = prefix.position;
        let recipe = prefix.recipe;
        let stat = recipe.stats[i];
        let remaining = 4 - i;
        let extras = prefix.extras;
        let first = if remaining == 1 { extras } else { 0 };
        (first..=extras).flat_map(move |hits| {
            // 给当前词条分到的强化数服从 Binomial(extras, 1 / remaining)。
            let misses = extras - hits;
            let combinations = FACTORIAL[extras as usize]
                / (FACTORIAL[hits as usize] * FACTORIAL[misses as usize]);
            let mut hit_log = combinations.ln() - f64::from(extras) * (remaining as f64).ln();
            if misses > 0 {
                hit_log += f64::from(misses) * ((remaining - 1) as f64).ln();
            }
            let count = 1 + hits;
            self.outcomes[count as usize].iter().map(move |outcome| {
                let value = rolls(stat)
                    .into_iter()
                    .zip(outcome.counts)
                    .map(|(roll, n)| roll * f64::from(n))
                    .sum();
                let mut next = prefix;
                next.position += 1;
                next.extras = misses;
                next.values[i] = value;
                next.score += contribution(self.tiers, stat, recipe.weights[i], count, value);
                (next, hit_log + outcome.log_mass)
            })
        })
    }

    fn log_probability(&self, prefix: Prefix<'_>, cache: &mut CompletionMass) -> f64 {
        if let Some(mass) = self.terminal_mass(prefix) {
            return mass;
        }
        // 对同一词条组合，后续达标概率只取决于已处理的位置、剩余强化次数和当前得分。
        let key = (prefix.position, prefix.extras, prefix.score.to_bits());
        if let Some(&mass) = cache.get(&key) {
            return mass;
        }
        let mut log_sum = f64::NEG_INFINITY;
        for (child, log_prior) in self.branches(prefix) {
            let mass = log_prior + self.log_probability(child, cache);
            if mass.is_finite() {
                let high = log_sum.max(mass);
                log_sum = high + (log_sum.min(mass) - high).exp().ln_1p();
            }
        }
        cache.insert(key, log_sum);
        log_sum
    }

    fn draw(
        &self,
        piece: &PieceSearch,
        search: &mut Search<'_>,
    ) -> Result<Candidate, GenerationError> {
        let mut roots = Vec::new();
        let mut cache = CompletionMass::new();
        for recipe in &piece.recipes {
            cache.clear();
            for &count in &piece.counts {
                let prefix = Prefix::new(recipe, count);
                // 两种起始条数的先验权重为 5:2；只有一种时，它的常数权重可约去。
                let prior = recipe.mass * if count == 8 { 5.0 } else { 2.0 };
                roots.push((
                    prefix,
                    prior.ln() + self.log_probability(prefix, &mut cache),
                ));
            }
        }
        let mut prefix = weighted_branch(roots.into_iter(), search)?;
        cache.clear();
        while self.terminal_mass(prefix).is_none() {
            let branches = self
                .branches(prefix)
                .map(|(child, prior)| (child, prior + self.log_probability(child, &mut cache)));
            prefix = weighted_branch(branches, search)?;
        }
        Ok(prefix.finish(self.tiers, search))
    }
}

// 指数竞赛在分支间按概率选取。对数形式保留精确分数所需的极小正权重。
fn weighted_branch<T>(
    branches: impl Iterator<Item = (T, f64)>,
    search: &mut Search<'_>,
) -> Result<T, GenerationError> {
    let mut best = f64::NEG_INFINITY;
    let mut selected = None;
    for (branch, log_mass) in branches.filter(|(_, mass)| mass.is_finite()) {
        let uniform = search.unit().max(f64::MIN_POSITIVE);
        let priority = log_mass - (-uniform.ln()).ln();
        if priority > best {
            best = priority;
            selected = Some(branch);
        }
    }
    selected.ok_or(GenerationError::NoSolution)
}

impl PieceSearch {
    pub(in crate::relics) fn sample(
        &self,
        bounds: Bounds,
        tiers: [f64; 3],
        search: &mut Search<'_>,
    ) -> Result<Candidate, GenerationError> {
        let last = self.recipes.last().ok_or(GenerationError::NoSolution)?;
        let total: f64 = self.recipes.iter().map(|r| r.mass).sum();
        let mut draw = search.unit() * total;
        let recipe = self
            .recipes
            .iter()
            .find(|r| {
                draw -= r.mass;
                draw < 0.0
            })
            .unwrap_or(last);
        let count = if self.counts.len() == 1 {
            self.counts[0]
        } else if search.index(7) < 5 {
            8
        } else {
            9
        };
        let candidate = Prefix::new(recipe, count).finish(tiers, search);
        if contains(bounds, candidate.score) {
            return Ok(candidate);
        }

        ConditionalDraw::new(bounds, tiers).draw(self, search)
    }
}
