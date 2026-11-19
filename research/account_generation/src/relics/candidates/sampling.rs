//! 按主副词条组合、强化落点和数值档位的原始概率，在给定分数范围内抽一件成品。

use std::collections::HashMap;

use super::{
    Bounds, Candidate, EPSILON, GenerationError, PieceSearch, Recipe, Search, Sub, contains,
    extremes, rolls,
};

/// 0 到 6 的阶乘，用于计算多项档位分配概率。
const FACTORIAL: [f64; 7] = [1.0, 1.0, 2.0, 6.0, 24.0, 120.0, 720.0];

/// 一种副词条强化分配的前缀状态。
#[derive(Clone, Copy)]
struct Prefix<'a> {
    /// 当前四副词条组合配方。
    recipe: &'a Recipe,
    /// 下一条尚未处理的副词条索引，最大为 4。
    position: usize,
    /// 尚未分配给副词条的强化次数。
    extras: u8,
    /// 已处理副词条的数值总和，百分比属性以比例值表示。
    values: [f64; 4],
    /// 已处理副词条贡献的有效词条分。
    score: f64,
}

/// 创建并完成一条强化分配路径。
impl<'a> Prefix<'a> {
    /// 创建起始副词条数对应的根状态；总次数为 8 或 9。
    fn new(recipe: &'a Recipe, count: u8) -> Self {
        Self {
            recipe,
            position: 0,
            extras: count - 4,
            values: [0.0; 4],
            score: 0.0,
        }
    }

    /// 剩余分配都符合区间时，每次强化独立等概率选择一个待处理副词条，再按档位权重抽其数值。
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
            values: self.values,
            score: self.score,
        }
    }
}

/// 计算属性的有效词条贡献；纯中档时按次数计分，其余档位按实际数值归一化。
fn contribution(tiers: [f64; 3], stat: Sub, weight: f64, count: u8, value: f64) -> f64 {
    if tiers[0] == 0.0 && tiers[2] == 0.0 {
        weight * f64::from(count)
    } else {
        weight * (value / rolls(stat)[1])
    }
}

/// 一种强化次数对应的低中高档次数和概率质量。
pub(super) struct TierOutcome {
    /// 低、中、高档强化次数，三项之和等于本次强化次数。
    pub(super) counts: [u8; 3],
    /// 该次数组合相对概率的自然对数。
    pub(super) log_mass: f64,
}

/// 按有效分数区间对原始强化概率进行条件抽样的搜索器。
struct ConditionalDraw {
    /// 成品分数必须满足的闭区间。
    bounds: Bounds,
    /// 低、中、高档的抽样权重。
    tiers: [f64; 3],
    /// 权重大于零的档位索引端点，用于可达分数边界。
    extremes: (usize, usize),
    /// 按本次强化次数索引的多项分布结果。
    outcomes: [Vec<TierOutcome>; 7],
}

/// 枚举每个强化次数下所有可用档位次数组合及其多项分布对数概率。
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

/// 以副词条位置、剩余强化次数和累计分数位模式缓存完成概率。
type CompletionMass = HashMap<(usize, u8, u64), f64>;

/// 对给定分数区间计算并抽取合法强化路径。
impl ConditionalDraw {
    /// 初始化档位组合表和可达分数边界。
    fn new(bounds: Bounds, tiers: [f64; 3]) -> Self {
        Self {
            bounds,
            tiers,
            extremes: extremes(tiers),
            outcomes: tier_outcomes(tiers),
        }
    }

    // 整段落入区间时，所有后续抽法都合法；整段在区间外时，可行概率为零。
    /// 判断前缀能否立即终止，并返回其完成路径的对数质量。
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

    /// 产生下一条属性的强化次数及档位组合，并给出分支对数概率。
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

    /// 递归计算当前前缀最终落入目标分数区间的对数概率质量。
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

    /// 按各合法后续路径的原始概率质量逐步抽取完整候选。
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

/// 用对数形式的指数竞赛抽取分支，保留精确分数条件下极小的正权重。
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

/// 从遗器搜索器中抽取满足分数区间的完整候选。
impl PieceSearch {
    /// 先尝试原始先验抽样；不符合区间时按条件概率路径重新抽样。
    pub(in crate::relics) fn sample(
        &self,
        bounds: Bounds,
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
        let candidate = Prefix::new(recipe, count).finish(self.tiers, search);
        if contains(bounds, candidate.score) {
            return Ok(candidate);
        }

        ConditionalDraw::new(bounds, self.tiers).draw(self, search)
    }
}
