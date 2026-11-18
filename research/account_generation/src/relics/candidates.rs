use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicInstance, RelicMainStat as Main, RelicSlot, RelicSubStat as Sub};

use crate::{GenerationError, Search};

use super::{Bounds, EPSILON, RelicRequest, RelicSpec, contains, feasible, intersect, set_range};

const STATS: [Sub; 12] = [
    Sub::Hp,
    Sub::Atk,
    Sub::Def,
    Sub::HpPercent,
    Sub::AtkPercent,
    Sub::DefPercent,
    Sub::Spd,
    Sub::CritRate,
    Sub::CritDmg,
    Sub::EffectHitRate,
    Sub::EffectRes,
    Sub::BreakEffect,
];

const MAIN_STATS: [Main; 19] = [
    Main::Hp,
    Main::Atk,
    Main::HpPercent,
    Main::AtkPercent,
    Main::DefPercent,
    Main::Spd,
    Main::CritRate,
    Main::CritDmg,
    Main::EffectHitRate,
    Main::OutgoingHealingBoost,
    Main::BreakEffect,
    Main::EnergyRegenerationRate,
    Main::PhysicalDmgBoost,
    Main::FireDmgBoost,
    Main::IceDmgBoost,
    Main::LightningDmgBoost,
    Main::WindDmgBoost,
    Main::QuantumDmgBoost,
    Main::ImaginaryDmgBoost,
];

pub(super) fn rolls(stat: Sub) -> [f64; 3] {
    match stat {
        Sub::Hp => [33.87004, 38.103798, 42.33755],
        Sub::Atk | Sub::Def => [16.935019, 19.051895, 21.168774],
        Sub::HpPercent | Sub::AtkPercent | Sub::EffectHitRate | Sub::EffectRes => {
            [0.034560003, 0.038880002, 0.0432]
        }
        Sub::DefPercent => [0.0432, 0.0486, 0.054],
        Sub::Spd => [2.0, 2.3, 2.6],
        Sub::CritRate => [0.02592, 0.029160001, 0.0324],
        Sub::CritDmg | Sub::BreakEffect => [0.05184, 0.058320003, 0.0648],
    }
}

fn excluded(main: Main) -> Option<Sub> {
    match main {
        Main::Hp => Some(Sub::Hp),
        Main::Atk => Some(Sub::Atk),
        Main::HpPercent => Some(Sub::HpPercent),
        Main::AtkPercent => Some(Sub::AtkPercent),
        Main::DefPercent => Some(Sub::DefPercent),
        Main::Spd => Some(Sub::Spd),
        Main::CritRate => Some(Sub::CritRate),
        Main::CritDmg => Some(Sub::CritDmg),
        Main::EffectHitRate => Some(Sub::EffectHitRate),
        Main::BreakEffect => Some(Sub::BreakEffect),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) struct Candidate {
    main: Main,
    stats: [Sub; 4],
    values: [f64; 4],
    weights: [f64; 4],
    pub(super) score: f64,
    zero_extras: u8,
}

/// 四种互异副词条及每种词条的生成参数。
struct Recipe {
    main: Main,
    stats: [Sub; 4],
    weights: [f64; 4],
}

impl Recipe {
    fn bounds(&self, count: u8, tiers: (usize, usize)) -> Bounds {
        self.tail_bounds(0, count - 4, tiers)
    }

    fn tail_bounds(&self, start: usize, extras: u8, tiers: (usize, usize)) -> Bounds {
        let scores = |tier: usize, opposite: usize, combine: fn(f64, f64) -> f64| {
            let mut values = (start..4).map(|index| {
                let roll = rolls(self.stats[index]);
                let weight = self.weights[index];
                let tier = if weight < 0.0 { opposite } else { tier };
                roll[tier] / roll[1] * weight
            });
            let Some(first) = values.next() else {
                return 0.0;
            };
            let (sum, extreme) = values.fold((first, first), |(sum, extreme), value| {
                (sum + value, combine(extreme, value))
            });
            sum + f64::from(extras) * extreme
        };
        (
            scores(tiers.0, tiers.1, f64::min),
            scores(tiers.1, tiers.0, f64::max),
        )
    }
}

pub(super) fn maximum_score(
    spec: &RelicSpec,
    request: &RelicRequest,
) -> Result<f64, GenerationError> {
    let recipes = recipes(
        spec,
        mainstats(spec),
        STATS,
        &request.effective_stat_weights,
    );
    let bounds = legal_bounds(&recipes, &roll_counts(spec), extremes(request.tier_weights));
    feasible(bounds)?;
    Ok(bounds.1)
}

fn mainstats(spec: &RelicSpec) -> Vec<Main> {
    if spec.mainstat.is_empty() {
        MAIN_STATS
            .into_iter()
            .filter(|stat| stat.supports_slot(spec.slot))
            .collect()
    } else {
        spec.mainstat.clone()
    }
}

fn roll_counts(spec: &RelicSpec) -> Vec<u8> {
    spec.initial_substat_count
        .map_or_else(|| vec![8, 9], |count| vec![count + 5])
}

fn extremes(tier_weights: [f64; 3]) -> (usize, usize) {
    let mut enabled = (0..3).filter(|&tier| tier_weights[tier] > 0.0);
    let first = enabled.next().expect("validated tier weights");
    (first, enabled.next_back().unwrap_or(first))
}

fn legal_bounds(recipes: &[Recipe], counts: &[u8], tiers: (usize, usize)) -> Bounds {
    recipes
        .iter()
        .flat_map(|recipe| counts.iter().map(move |&count| recipe.bounds(count, tiers)))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(lo, hi), (min, max)| (lo.min(min), hi.max(max)),
        )
}

fn recipes(
    spec: &RelicSpec,
    mains: Vec<Main>,
    stats: [Sub; 12],
    weights: &BTreeMap<Sub, f64>,
) -> Vec<Recipe> {
    let weight = |stat: Sub| weights.get(&stat).copied().unwrap_or_default();
    let mut examined = BTreeSet::new();
    let mut recipes = Vec::new();
    for main in mains {
        let keys: Vec<_> = stats
            .iter()
            .copied()
            .filter(|&stat| Some(stat) != excluded(main))
            .collect();
        for a in 0..keys.len() {
            for b in a + 1..keys.len() {
                for c in b + 1..keys.len() {
                    for d in c + 1..keys.len() {
                        let mut stats = [keys[a], keys[b], keys[c], keys[d]];
                        if !spec
                            .required_substats
                            .iter()
                            .all(|stat| stats.contains(stat))
                        {
                            continue;
                        }
                        stats.sort();
                        let effective: Vec<_> = stats
                            .iter()
                            .copied()
                            .filter(|&stat| weight(stat) != 0.0)
                            .collect();
                        if examined.insert(effective) {
                            recipes.push(Recipe {
                                main,
                                stats,
                                weights: stats.map(weight),
                            });
                        }
                    }
                }
            }
        }
    }
    recipes
}

pub(super) struct PieceSearch {
    set: String,
    slot: RelicSlot,
    recipes: Vec<Recipe>,
    counts: Vec<u8>,
    tiers: [f64; 3],
    pub(super) upper: f64,
    pub(super) bounds: Bounds,
}

impl PieceSearch {
    pub(super) fn new(
        spec: &RelicSpec,
        request: &RelicRequest,
        search: &mut Search<'_>,
    ) -> Result<Self, GenerationError> {
        let mut mains = mainstats(spec);
        search.shuffle(&mut mains);
        let mut stats = STATS;
        search.shuffle(&mut stats);
        let recipes = recipes(spec, mains, stats, &request.effective_stat_weights);
        let mut counts = roll_counts(spec);
        if counts.len() == 2 && search.index(7) >= 5 {
            counts.swap(0, 1);
        }
        let legal = legal_bounds(&recipes, &counts, extremes(request.tier_weights));
        let bounds = intersect(
            legal,
            super::score_bounds(spec.min_effective_rolls, spec.max_effective_rolls),
        );
        feasible(bounds)?;
        let set = spec.set_id.clone().unwrap_or_else(|| {
            let sets = set_range(spec.slot);
            let first = *sets.start();
            (first + search.index(sets.count()) as u16).to_string()
        });
        Ok(Self {
            set,
            slot: spec.slot,
            recipes,
            counts,
            tiers: request.tier_weights,
            upper: legal.1,
            bounds,
        })
    }

    pub(super) fn has_middle(&self) -> bool {
        self.tiers[1] > 0.0
    }

    pub(super) fn is_middle_only(&self) -> bool {
        self.has_middle() && self.tiers[0] == 0.0 && self.tiers[2] == 0.0
    }

    pub(super) fn candidates(
        &self,
        bounds: Bounds,
        desired: Option<f64>,
        search: &mut Search<'_>,
    ) -> Candidates<'_> {
        let tiers = if desired.is_some() {
            self.tiers
        } else {
            [0.0, 1.0, 0.0]
        };
        let extremes = extremes(tiers);
        let mut pending = Vec::new();
        for &count in &self.counts {
            for recipe in &self.recipes {
                let bounds = intersect(bounds, recipe.bounds(count, extremes));
                if bounds.0 <= bounds.1 + EPSILON {
                    pending.push(Assignment {
                        recipe,
                        position: 0,
                        remaining: count,
                        zero_extras: 0,
                        score: 0.0,
                        values: [0.0; 4],
                        distance: desired.map_or(0.0, |goal| distance(bounds, goal)),
                    });
                }
            }
        }
        next_first(&mut pending, desired.is_some());
        // 同分成品保留首个候选。独立抽取每种副词条的顺序，让同分分配也有机会入选。
        let count_orders = std::array::from_fn(|_| {
            let mut counts = [1, 2, 3, 4, 5, 6];
            search.shuffle(&mut counts);
            counts
        });
        Candidates {
            pending,
            count_orders,
            bounds,
            extremes,
            desired,
            distributions: distributions(tiers, search),
            examined: BTreeSet::new(),
        }
    }

    pub(super) fn materialize(
        &self,
        mut candidate: Candidate,
        search: &mut Search<'_>,
    ) -> RelicInstance {
        // 搜索按得分合并零权重词条。确定成品后，再逐次抽取它们的强化去向和档位。
        let unscored: Vec<_> = (0..4).filter(|&i| candidate.weights[i] == 0.0).collect();
        if !unscored.is_empty() {
            let mut counts = [1u8; 4];
            for _ in 0..candidate.zero_extras {
                counts[unscored[search.index(unscored.len())]] += 1;
            }
            for i in unscored {
                candidate.values[i] = self.sample_rolls(candidate.stats[i], counts[i], search);
            }
        }
        RelicInstance {
            instance_id: String::new(),
            set_id: self.set.clone(),
            slot: self.slot,
            rarity: 5,
            level: 15,
            mainstat: candidate.main,
            substats: candidate.stats.into_iter().zip(candidate.values).collect(),
        }
    }

    fn sample_rolls(&self, stat: Sub, count: u8, search: &mut Search<'_>) -> f64 {
        let largest = self.tiers.into_iter().fold(0.0, f64::max);
        let [low, middle, high] = self.tiers.map(|weight| weight / largest);
        let values = rolls(stat);
        (0..count)
            .map(|_| {
                let draw = search.unit() * (low + middle + high);
                if low > 0.0 && draw < low {
                    values[0]
                } else if high == 0.0 || draw < low + middle {
                    values[1]
                } else {
                    values[2]
                }
            })
            .sum()
    }

    /// 用允许的档位重抽中档候选的强化数值，保留每个副词条的强化次数。
    pub(super) fn sample_tiers(
        &self,
        mut candidate: Candidate,
        bounds: Bounds,
        search: &mut Search<'_>,
    ) -> Candidate {
        let original_score = candidate.score;
        let accepts = |score: f64| {
            if bounds.1 - bounds.0 <= 2.0 * EPSILON + 1e-12 {
                // 精确目标保留已选分数，舍入误差不作为可分配的分数余量。
                let rounding = 8.0 * f64::EPSILON * original_score.abs().max(1.0);
                (score - original_score).abs() <= rounding
            } else {
                contains(bounds, score)
            }
        };
        let mut proposed = candidate;
        let mut changes = [0.0; 4];
        for (index, change) in changes.iter_mut().enumerate() {
            let middle_value = rolls(candidate.stats[index])[1];
            let count = (candidate.values[index] / middle_value).round();
            let value = self.sample_rolls(candidate.stats[index], count as u8, search);
            proposed.values[index] = value;
            *change = candidate.weights[index] * (value / middle_value - count);
        }
        proposed.score += changes.iter().sum::<f64>();
        if accepts(proposed.score) {
            return proposed;
        }

        // 整件超出范围时，仍接受符合约束的单个词条数值，包括零权重词条。
        let mut indices = [0, 1, 2, 3];
        search.shuffle(&mut indices);
        for index in indices {
            let score = candidate.score + changes[index];
            if accepts(score) {
                candidate.values[index] = proposed.values[index];
                candidate.score = score;
            }
        }
        candidate
    }
}

/// 逐件产生候选。待考察的分配保存在向量中，调用方可以暂停以检查后续部位。
pub(super) struct Candidates<'a> {
    pending: Vec<Assignment<'a>>,
    count_orders: [[u8; 6]; 4],
    bounds: Bounds,
    extremes: (usize, usize),
    desired: Option<f64>,
    distributions: DistributionTable,
    examined: BTreeSet<u64>,
}

#[derive(Clone, Copy)]
struct Assignment<'a> {
    recipe: &'a Recipe,
    position: usize,
    remaining: u8,
    zero_extras: u8,
    score: f64,
    values: [f64; 4],
    distance: f64,
}

impl Iterator for Candidates<'_> {
    type Item = Candidate;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(assignment) = self.pending.pop() {
            if assignment.position < 4 {
                self.expand(assignment);
                continue;
            }
            let score = assignment.score;
            if !contains(self.bounds, score) {
                continue;
            }
            let key = if self.desired.is_none() {
                let snapped = (score * 1e12).round() / 1e12;
                if snapped.is_finite() { snapped } else { score }
            } else {
                score
            };
            if self.examined.insert(key.to_bits()) {
                return Some(Candidate {
                    main: assignment.recipe.main,
                    stats: assignment.recipe.stats,
                    weights: assignment.recipe.weights,
                    values: assignment.values,
                    score,
                    zero_extras: assignment.zero_extras,
                });
            }
        }
        None
    }
}

impl<'a> Candidates<'a> {
    fn expand(&mut self, assignment: Assignment<'a>) {
        let Assignment {
            recipe,
            position,
            remaining,
            score,
            ..
        } = assignment;
        let stats_left = (4 - position) as u8;
        let maximum = remaining - stats_left + 1;
        let minimum = if stats_left == 1 { maximum } else { 1 };
        let weight = recipe.weights[position];
        let equivalent_zero = weight == 0.0 && recipe.weights[position + 1..].contains(&0.0);
        let rolls = rolls(recipe.stats[position]);
        let start = self.pending.len();

        let range = minimum..=if equivalent_zero { minimum } else { maximum };
        for count in self.count_orders[position]
            .into_iter()
            .filter(|count| range.contains(count))
        {
            let distributions = &self.distributions[usize::from(count)];
            let distributions = if weight == 0.0 {
                &distributions[..1]
            } else {
                distributions
            };
            let (low, high) = recipe.tail_bounds(position + 1, maximum - count, self.extremes);
            for distribution in distributions {
                let value: f64 = rolls
                    .into_iter()
                    .zip(distribution)
                    .map(|(roll, &count)| roll * f64::from(count))
                    .sum();
                let contribution = if self.desired.is_some() {
                    weight * (value / rolls[1])
                } else {
                    weight * f64::from(count)
                };
                let score = score + contribution;
                let bounds = intersect(self.bounds, (score + low, score + high));
                if bounds.0 <= bounds.1 + EPSILON {
                    let mut values = assignment.values;
                    values[position] = value;
                    self.pending.push(Assignment {
                        recipe,
                        position: position + 1,
                        remaining: remaining - count,
                        zero_extras: assignment.zero_extras
                            + if weight == 0.0 { count - 1 } else { 0 },
                        score,
                        values,
                        distance: self.desired.map_or(0.0, |goal| distance(bounds, goal)),
                    });
                }
            }
        }
        next_first(&mut self.pending[start..], self.desired.is_some());
    }
}

fn next_first(assignments: &mut [Assignment<'_>], ranked: bool) {
    if ranked {
        assignments.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    }
    assignments.reverse();
}

fn distance((min, max): Bounds, goal: f64) -> f64 {
    (min - goal).max(goal - max).max(0.0)
}

type DistributionTable = [Vec<[u8; 3]>; 7];

fn distributions(tier_weights: [f64; 3], search: &mut Search<'_>) -> DistributionTable {
    std::array::from_fn(|count| {
        let count = count as u8;
        let mut options = Vec::new();
        for low in 0..=count {
            for middle in 0..=count - low {
                let counts = [low, middle, count - low - middle];
                if counts
                    .iter()
                    .zip(tier_weights)
                    .any(|(&count, weight)| count > 0 && weight == 0.0)
                {
                    continue;
                }
                // 随机排序保留多项式概率，包括同一数值分配的不同强化顺序。
                let weight: f64 = counts
                    .iter()
                    .zip(tier_weights)
                    .filter(|&(&count, _)| count > 0)
                    .map(|(&count, weight)| {
                        f64::from(count) * weight.ln()
                            - (1..=count).map(|n| f64::from(n).ln()).sum::<f64>()
                    })
                    .sum();
                let draw = (1.0 - search.unit()).ln();
                options.push((counts, weight - (-draw).ln()));
            }
        }
        options.sort_by(|a, b| b.1.total_cmp(&a.1));
        options.into_iter().map(|(counts, _)| counts).collect()
    })
}
