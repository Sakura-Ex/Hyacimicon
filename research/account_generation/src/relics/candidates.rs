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
        let extremes = |tier: usize, minimum: bool| {
            let scores = self.stats.map(|stat| {
                let values = rolls(stat);
                values[tier] / values[1]
            });
            let scores = std::array::from_fn::<_, 4, _>(|i| scores[i] * self.weights[i]);
            let combine = if minimum { f64::min } else { f64::max };
            scores.iter().sum::<f64>()
                + f64::from(count - 4) * scores.into_iter().reduce(combine).unwrap_or_default()
        };
        (extremes(tiers.0, true), extremes(tiers.1, false))
    }
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
        spec.validate()?;
        let mut mains = if spec.mainstat.is_empty() {
            MAIN_STATS
                .into_iter()
                .filter(|&stat| stat.supports_slot(spec.slot))
                .collect()
        } else {
            spec.mainstat.clone()
        };
        search.shuffle(&mut mains);
        let mut stats = STATS;
        search.shuffle(&mut stats);
        let weight = |stat: Sub| {
            request
                .effective_stat_weights
                .get(&stat)
                .copied()
                .unwrap_or_default()
        };
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
        let mut counts = spec
            .initial_substat_count
            .map_or_else(|| vec![8, 9], |count| vec![count + 5]);
        if counts.len() == 2 && search.index(7) >= 5 {
            counts.swap(0, 1);
        }
        let enabled: Vec<_> = (0..3)
            .filter(|&tier| request.tier_weights[tier] > 0.0)
            .collect();
        let tiers = (enabled[0], enabled[enabled.len() - 1]);
        let legal = recipes
            .iter()
            .flat_map(|recipe| counts.iter().map(move |&count| recipe.bounds(count, tiers)))
            .fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(lo, hi), (min, max)| (lo.min(min), hi.max(max)),
            );
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

    pub(super) fn candidates(
        &self,
        bounds: Bounds,
        desired: f64,
        middle: bool,
        search: &mut Search<'_>,
    ) -> Result<Vec<Candidate>, GenerationError> {
        let mut result = BTreeMap::new();
        let tiers = if middle { [0.0, 1.0, 0.0] } else { self.tiers };
        let enabled: Vec<_> = (0..3).filter(|&index| tiers[index] > 0.0).collect();
        let extremes = (enabled[0], enabled[enabled.len() - 1]);
        let distributions = distributions(tiers, search);
        for (position, &count) in self.counts.iter().enumerate() {
            let preferred_initial = position == 0;
            for recipe in &self.recipes {
                let recipe_bounds = recipe.bounds(count, extremes);
                if recipe_bounds.0 > bounds.1 + EPSILON || recipe_bounds.1 < bounds.0 - EPSILON {
                    continue;
                }
                let mut collector = CandidateSearch {
                    recipe,
                    distributions: &distributions,
                    bounds,
                    extremes,
                    preferred_initial,
                    result: &mut result,
                    middle,
                    search,
                };
                collector.enumerate(0, count, 0.0, [0.0; 4])?;
            }
        }
        let mut candidates: Vec<_> = result.into_values().collect();
        candidates.sort_by(|a, b| {
            (a.score - desired)
                .abs()
                .total_cmp(&(b.score - desired).abs())
                .then_with(|| b.preferred_initial.cmp(&a.preferred_initial))
        });
        Ok(candidates)
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
        if self.is_middle_only() {
            return candidate;
        }
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

struct CandidateSearch<'a, 'b, 'rng> {
    recipe: &'a Recipe,
    distributions: &'a DistributionTable,
    bounds: Bounds,
    extremes: (usize, usize),
    preferred_initial: bool,
    result: &'b mut BTreeMap<u64, Candidate>,
    middle: bool,
    search: &'b mut Search<'rng>,
}

impl CandidateSearch<'_, '_, '_> {
    fn enumerate(
        &mut self,
        position: usize,
        remaining: u8,
        score: f64,
        mut values: [f64; 4],
    ) -> Result<(), GenerationError> {
        if position == 4 {
            if contains(self.bounds, score) {
                let candidate = Candidate {
                    main: self.recipe.main,
                    stats: self.recipe.stats,
                    values,
                    weights: self.recipe.weights,
                    score,
                    preferred_initial: self.preferred_initial,
                };
                let key = if self.middle {
                    let snapped = (score * 1e12).round() / 1e12;
                    if snapped.is_finite() {
                        snapped.to_bits()
                    } else {
                        score.to_bits()
                    }
                } else {
                    score.to_bits()
                };
                if let std::collections::btree_map::Entry::Vacant(entry) = self.result.entry(key) {
                    if !self.middle {
                        self.search.consider()?;
                    }
                    entry.insert(candidate);
                }
            }
            return Ok(());
        }
        let stats_left = (4 - position) as u8;
        let extras = remaining - stats_left;
        let tail = self.tail_bounds(position, extras);
        if score + tail.0 > self.bounds.1 + EPSILON || score + tail.1 < self.bounds.0 - EPSILON {
            return Ok(());
        }
        let stat = self.recipe.stats[position];
        let weight = self.recipe.weights[position];
        let maximum = extras + 1;
        let minimum = if stats_left == 1 { maximum } else { 1 };
        let equivalent_zero = weight == 0.0 && self.recipe.weights[position + 1..].contains(&0.0);
        for count in minimum..=if equivalent_zero { minimum } else { maximum } {
            let distributions = &self.distributions[usize::from(count)];
            let distributions = if weight == 0.0 {
                &distributions[..1]
            } else {
                distributions
            };
            let rolls = rolls(stat);
            for distribution in distributions {
                let value: f64 = rolls
                    .into_iter()
                    .zip(distribution)
                    .map(|(roll, &count)| roll * f64::from(count))
                    .sum();
                values[position] = value;
                let contribution = if self.middle {
                    weight * f64::from(count)
                } else {
                    weight * (value / rolls[1])
                };
                self.enumerate(
                    position + 1,
                    remaining - count,
                    score + contribution,
                    values,
                )?;
            }
        }
        Ok(())
    }

    fn tail_bounds(&self, start: usize, extras: u8) -> Bounds {
        let scores = |tier: usize, combine: fn(f64, f64) -> f64| {
            let scores: Vec<_> = (start..4)
                .map(|index| {
                    let roll = rolls(self.recipe.stats[index]);
                    roll[tier] / roll[1] * self.recipe.weights[index]
                })
                .collect();
            scores.iter().sum::<f64>()
                + f64::from(extras) * scores.into_iter().reduce(combine).unwrap_or_default()
        };
        (
            scores(self.extremes.0, f64::min),
            scores(self.extremes.1, f64::max),
        )
    }
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
