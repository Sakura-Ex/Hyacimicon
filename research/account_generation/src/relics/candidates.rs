use std::{
    collections::{BTreeMap, BTreeSet},
    ops::ControlFlow,
};

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
    preferred_initial: bool,
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
        let scores = |tier: usize, combine: fn(f64, f64) -> f64| {
            let mut values = (start..4).map(|index| {
                let roll = rolls(self.stats[index]);
                roll[tier] / roll[1] * self.weights[index]
            });
            let Some(first) = values.next() else {
                return 0.0;
            };
            let (sum, extreme) = values.fold((first, first), |(sum, extreme), value| {
                (sum + value, combine(extreme, value))
            });
            sum + f64::from(extras) * extreme
        };
        (scores(tiers.0, f64::min), scores(tiers.1, f64::max))
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
                            .filter(|&stat| weight(stat) > 0.0)
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
            (
                spec.min_effective_rolls.unwrap_or_default(),
                spec.max_effective_rolls.unwrap_or(f64::INFINITY),
            ),
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

    pub(super) fn distributions(&self, middle: bool, search: &mut Search<'_>) -> DistributionTable {
        distributions(if middle { [0.0, 1.0, 0.0] } else { self.tiers }, search)
    }

    pub(super) fn middle_candidates(
        &self,
        bounds: Bounds,
        desired: f64,
        distributions: &DistributionTable,
    ) -> Vec<Candidate> {
        let mut result = BTreeMap::new();
        let _: ControlFlow<()> = self.visit_candidates(bounds, None, distributions, |candidate| {
            let score = candidate.score;
            let snapped = (score * 1e12).round() / 1e12;
            let key = if snapped.is_finite() { snapped } else { score };
            result.entry(key.to_bits()).or_insert(candidate);
            ControlFlow::Continue(())
        });
        let mut candidates: Vec<_> = result.into_values().collect();
        candidates.sort_by(|a, b| {
            (a.score - desired)
                .abs()
                .total_cmp(&(b.score - desired).abs())
                .then_with(|| b.preferred_initial.cmp(&a.preferred_initial))
        });
        candidates
    }

    pub(super) fn visit_full_candidates<B>(
        &self,
        bounds: Bounds,
        desired: f64,
        distributions: &DistributionTable,
        mut visit: impl FnMut(Candidate) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        let mut examined = BTreeSet::new();
        self.visit_candidates(bounds, Some(desired), distributions, |candidate| {
            if examined.insert(candidate.score.to_bits()) {
                visit(candidate)
            } else {
                ControlFlow::Continue(())
            }
        })
    }

    fn visit_candidates<B>(
        &self,
        bounds: Bounds,
        desired: Option<f64>,
        distributions: &DistributionTable,
        mut visit: impl FnMut(Candidate) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        let extremes = desired.map_or((1, 1), |_| extremes(self.tiers));
        let mut options = Vec::new();
        for (position, &count) in self.counts.iter().enumerate() {
            let preferred_initial = position == 0;
            for recipe in &self.recipes {
                let recipe_bounds = recipe.bounds(count, extremes);
                if recipe_bounds.0 > bounds.1 + EPSILON || recipe_bounds.1 < bounds.0 - EPSILON {
                    continue;
                }
                let search = CandidateSearch {
                    recipe,
                    distributions,
                    bounds,
                    extremes,
                    preferred_initial,
                    desired,
                };
                if let Some(goal) = desired {
                    options.push((
                        distance(intersect(bounds, recipe_bounds), goal),
                        search,
                        count,
                    ));
                } else {
                    search.enumerate(0, count, 0.0, [0.0; 4], &mut visit)?;
                }
            }
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, search, count) in options {
            search.enumerate(0, count, 0.0, [0.0; 4], &mut visit)?;
        }
        ControlFlow::Continue(())
    }

    pub(super) fn materialize(&self, candidate: &Candidate) -> RelicInstance {
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

    /// 用允许的档位重抽中档候选的强化数值，保留每个副词条的强化次数。
    pub(super) fn sample_tiers(
        &self,
        mut candidate: Candidate,
        bounds: Bounds,
        search: &mut Search<'_>,
    ) -> Candidate {
        let largest = self.tiers.into_iter().reduce(f64::max).unwrap_or_default();
        let [low, middle, high] = self.tiers.map(|weight| weight / largest);
        let weight_sum = low + middle + high;
        let original_score = candidate.score;
        let accepts = |score: f64| {
            if bounds.1 - bounds.0 <= EPSILON {
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
            let [low_value, middle_value, high_value] = rolls(candidate.stats[index]);
            let count = (candidate.values[index] / middle_value).round();
            let value: f64 = (0..count as u8)
                .map(|_| {
                    let draw = search.unit() * weight_sum;
                    if low > 0.0 && draw < low {
                        low_value
                    } else if high == 0.0 || draw < low + middle {
                        middle_value
                    } else {
                        high_value
                    }
                })
                .sum();
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

struct CandidateSearch<'a> {
    recipe: &'a Recipe,
    distributions: &'a DistributionTable,
    bounds: Bounds,
    extremes: (usize, usize),
    preferred_initial: bool,
    desired: Option<f64>,
}

impl CandidateSearch<'_> {
    fn enumerate<B>(
        &self,
        position: usize,
        remaining: u8,
        score: f64,
        mut values: [f64; 4],
        visit: &mut impl FnMut(Candidate) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        if position == 4 {
            if contains(self.bounds, score) {
                return visit(Candidate {
                    main: self.recipe.main,
                    stats: self.recipe.stats,
                    values,
                    weights: self.recipe.weights,
                    score,
                    preferred_initial: self.preferred_initial,
                });
            }
            return ControlFlow::Continue(());
        }
        let stats_left = (4 - position) as u8;
        let extras = remaining - stats_left;
        let tail = self.recipe.tail_bounds(position, extras, self.extremes);
        if score + tail.0 > self.bounds.1 + EPSILON || score + tail.1 < self.bounds.0 - EPSILON {
            return ControlFlow::Continue(());
        }
        let stat = self.recipe.stats[position];
        let weight = self.recipe.weights[position];
        let maximum = extras + 1;
        let minimum = if stats_left == 1 { maximum } else { 1 };
        let equivalent_zero = weight == 0.0 && self.recipe.weights[position + 1..].contains(&0.0);
        let rolls = rolls(stat);
        let mut options = Vec::new();
        for count in minimum..=if equivalent_zero { minimum } else { maximum } {
            let distributions = &self.distributions[usize::from(count)];
            let distributions = if weight == 0.0 {
                &distributions[..1]
            } else {
                distributions
            };
            for distribution in distributions {
                let value: f64 = rolls
                    .into_iter()
                    .zip(distribution)
                    .map(|(roll, &count)| roll * f64::from(count))
                    .sum();
                if let Some(goal) = self.desired {
                    let next_score = score + weight * (value / rolls[1]);
                    let (low, high) =
                        self.recipe
                            .tail_bounds(position + 1, maximum - count, self.extremes);
                    let bounds = intersect(self.bounds, (next_score + low, next_score + high));
                    if bounds.0 <= bounds.1 + EPSILON {
                        options.push((distance(bounds, goal), count, value, next_score));
                    }
                } else {
                    values[position] = value;
                    self.enumerate(
                        position + 1,
                        remaining - count,
                        score + weight * f64::from(count),
                        values,
                        visit,
                    )?;
                }
            }
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, count, value, next_score) in options {
            values[position] = value;
            self.enumerate(position + 1, remaining - count, next_score, values, visit)?;
        }
        ControlFlow::Continue(())
    }
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
