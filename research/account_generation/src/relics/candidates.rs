//! 建立遗器主副词条组合，并计算分数范围内的候选概率。

use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicInstance, RelicMainStat as Main, RelicSlot, RelicSubStat as Sub};

use crate::{GenerationError, Search};

use super::{Bounds, EPSILON, RelicRequest, RelicSpec, contains, feasible, intersect, set_range};

/// 按强化档位的多项分布进行条件抽样。
mod sampling;

/// 全部可作为遗器副词条的十二种属性。
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

/// 遗器生成器支持的十九种主词条属性。
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

/// 返回副词条低、中、高三档单次强化值；百分比属性以比例值表示。
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

/// 返回与给定主词条同属性、不能同时出现在副词条中的属性。
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

/// 已完成的主词条、副词条数值和有效分候选。
#[derive(Clone, Copy)]
pub(super) struct Candidate {
    /// 该成品的主词条属性。
    main: Main,
    /// 四种互异副词条，位置与 `values` 对应。
    stats: [Sub; 4],
    /// 四种副词条最终数值，单位依属性而定，百分比以比例表示。
    values: [f64; 4],
    /// 按请求权重计算的有效词条分数。
    pub(super) score: f64,
}

/// 四种互异副词条及每种词条的生成参数。
struct Recipe {
    /// 此组合使用的主词条。
    main: Main,
    /// 此组合的四种副词条。
    stats: [Sub; 4],
    /// 四种副词条各自的计分权重。
    weights: [f64; 4],
    /// 给定主词条时，这个计分等价类占其合法四副词条组合的比例。
    mass: f64,
}

/// 计算指定强化次数和允许档位下，此组合可达到的得分范围。
impl Recipe {
    /// 返回四种副词条各强化一次后再分配剩余次数的完整得分范围。
    fn bounds(&self, count: u8, tiers: (usize, usize)) -> Bounds {
        self.tail_bounds(0, count - 4, tiers)
    }

    /// 计算从副词条索引 `start` 起、分配 `extras` 次强化的最小和最大贡献。
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

/// 求指定遗器条件和档位权重下所有合法成品的最高有效词条分。
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
    let bounds = legal_bounds(&recipes, &roll_counts(spec), extremes(request.tier_weights))?;
    Ok(bounds.1)
}

/// 返回请求主词条候选；空列表展开为该部位支持的全部主词条。
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

/// 返回可能的强化总次数；三条起始为八次，四条起始为九次。
fn roll_counts(spec: &RelicSpec) -> Vec<u8> {
    spec.initial_substat_count
        .map_or_else(|| vec![8, 9], |count| vec![count + 5])
}

/// 找出非零档位的最低和最高索引，供分数上下界计算使用。
fn extremes(tier_weights: [f64; 3]) -> (usize, usize) {
    let mut enabled = (0..3).filter(|&tier| tier_weights[tier] > 0.0);
    let first = enabled.next().expect("validated tier weights");
    (first, enabled.next_back().unwrap_or(first))
}

/// 汇总各主副词条组合及强化次数在允许档位下的整体分数边界。
fn legal_bounds(
    recipes: &[Recipe],
    counts: &[u8],
    tiers: (usize, usize),
) -> Result<Bounds, GenerationError> {
    recipes
        .iter()
        .flat_map(|recipe| counts.iter().map(move |&count| recipe.bounds(count, tiers)))
        .reduce(|(lo, hi), (min, max)| (lo.min(min), hi.max(max)))
        .ok_or(GenerationError::NoSolution)
}

/// 枚举符合必含属性条件的四副词条组合，并合并计分等价组合概率。
fn recipes(
    spec: &RelicSpec,
    mains: Vec<Main>,
    stats: [Sub; 12],
    weights: &BTreeMap<Sub, f64>,
) -> Vec<Recipe> {
    let weight = |stat: Sub| weights.get(&stat).copied().unwrap_or_default();
    let mut recipes: Vec<Recipe> = Vec::new();
    for main in mains {
        let start = recipes.len();
        let mut classes = BTreeMap::new();
        let mut combinations = 0;
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
                        // 同一主词条下，对得分等价的组合计数。打乱后的首个代表等概率选取零分填充词条。
                        let effective = stats
                            .iter()
                            .filter(|&&s| weight(s) != 0.0)
                            .fold(0u16, |mask, &s| mask | (1 << s as u32));
                        combinations += 1;
                        if let Some(&index) = classes.get(&effective) {
                            let recipe: &mut Recipe = &mut recipes[index];
                            recipe.mass += 1.0;
                        } else {
                            classes.insert(effective, recipes.len());
                            stats.sort_by_key(|&s| weight(s) == 0.0);
                            recipes.push(Recipe {
                                main,
                                stats,
                                weights: stats.map(weight),
                                mass: 1.0,
                            });
                        }
                    }
                }
            }
        }
        for recipe in &mut recipes[start..] {
            recipe.mass /= f64::from(combinations);
        }
    }
    recipes
}

/// 针对一个部位生成、枚举并评分合法遗器。
pub(super) struct PieceSearch {
    /// 生成时固定选定的套装 ID 字符串。
    set: String,
    /// 此搜索器对应的遗器部位。
    slot: RelicSlot,
    /// 符合主词条、必含副词条和权重条件的组合配方。
    recipes: Vec<Recipe>,
    /// 可选强化总次数；对应起始三条或四条。
    counts: Vec<u8>,
    /// 低、中、高档抽样权重。
    pub(super) tiers: [f64; 3],
    /// 所有合法候选的最高有效词条分。
    pub(super) upper: f64,
    /// 单件分数条件与合法分数边界的交集。
    pub(super) bounds: Bounds,
}

/// 构造遗器候选并提供搜索、抽样和实例化操作。
impl PieceSearch {
    /// 固定此件套装，打乱主、副词条的遍历顺序，并将合法分数范围与请求区间取交集。
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
        let legal = legal_bounds(&recipes, &counts, extremes(request.tier_weights))?;
        let bounds = intersect(
            super::with_roundoff(legal),
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

    /// 判断中档数值权重是否非零，以决定能否使用纯中档网格搜索。
    pub(super) fn has_middle(&self) -> bool {
        self.tiers[1] > 0.0
    }

    /// 未给目标时枚举纯中档的可达分数供网格搜索。给定目标时使用全部允许档位，优先尝试接近目标的分数。
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
                        score: 0.0,
                        distance: desired.map_or(0.0, |goal| distance(bounds, goal)),
                    });
                }
            }
        }
        next_first(&mut pending, desired.is_some());
        Candidates {
            pending,
            bounds,
            extremes,
            desired,
            distributions: distributions(tiers, search),
            examined: BTreeSet::new(),
        }
    }

    /// 将搜索候选转为五星 +15 遗器实例，实例 ID 留待账号组装时分配。
    pub(super) fn materialize(&self, candidate: Candidate) -> RelicInstance {
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

    /// 独立抽取指定属性的多次强化数值并求和；全中档时直接返回确定值。
    fn sample_rolls(tiers: [f64; 3], stat: Sub, count: u8, search: &mut Search<'_>) -> f64 {
        if tiers[0] == 0.0 && tiers[2] == 0.0 {
            return f64::from(count) * rolls(stat)[1];
        }
        let largest = tiers.into_iter().fold(0.0, f64::max);
        let [low, middle, high] = tiers.map(|weight| weight / largest);
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
}

/// 逐件产生候选。待考察的分配保存在向量中，调用方可以暂停以检查后续部位。
pub(super) struct Candidates<'a> {
    /// 尚待展开或检查的副词条强化分配状态栈。
    pending: Vec<Assignment<'a>>,
    /// 候选成品分数必须满足的闭区间。
    bounds: Bounds,
    /// 当前档位配置中允许出现的最低和最高档索引。
    extremes: (usize, usize),
    /// 可选的目标分数，用于优先枚举距离较近的分配。
    desired: Option<f64>,
    /// 每个剩余强化次数对应的档位组合列表。
    distributions: DistributionTable,
    /// 已返回的分数键，避免重复产生相同候选。
    examined: BTreeSet<u64>,
}

/// 一条副词条强化分配的中间状态。
#[derive(Clone, Copy)]
struct Assignment<'a> {
    /// 当前分配所属的主副词条组合。
    recipe: &'a Recipe,
    /// 下一项待分配强化的副词条索引，范围为 0 到 4。
    position: usize,
    /// 尚未分配给四种副词条的强化次数。
    remaining: u8,
    /// 已处理副词条贡献的累计分数。
    score: f64,
    /// 当前分支可达区间与目标分数之间的最短距离。
    distance: f64,
}

/// 按需展开强化分配并输出去重后的可行成品分数。
impl Iterator for Candidates<'_> {
    /// 迭代器输出有效词条分数。
    type Item = f64;

    /// 返回下一个满足区间且此前未返回的候选分数。
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
                (score * 1e12).round() / 1e12
            } else {
                score
            };
            if self.examined.insert(key.to_bits()) {
                return Some(score);
            }
        }
        None
    }
}

/// 展开候选强化分配的内部操作。
impl<'a> Candidates<'a> {
    /// 将一个强化分配前缀扩展为下一种副词条的合法强化次数与档位组合。
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
        for count in range {
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
                    self.pending.push(Assignment {
                        recipe,
                        position: position + 1,
                        remaining: remaining - count,
                        score,
                        distance: self.desired.map_or(0.0, |goal| distance(bounds, goal)),
                    });
                }
            }
        }
        next_first(&mut self.pending[start..], self.desired.is_some());
    }
}

/// 调整栈内顺序，使最近目标的分支在后续弹栈时优先展开。
fn next_first(assignments: &mut [Assignment<'_>], ranked: bool) {
    if ranked {
        assignments.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    }
    assignments.reverse();
}

/// 返回目标分数到闭区间的距离；目标落在区间内时距离为零。
fn distance((min, max): Bounds, goal: f64) -> f64 {
    (min - goal).max(goal - max).max(0.0)
}

/// 按强化次数索引的三档次数分布表，支持 0 到 6 次强化。
type DistributionTable = [Vec<[u8; 3]>; 7];

/// 为每种强化次数建立档位次数组合，并按抽样优先级排列。
fn distributions(tier_weights: [f64; 3], search: &mut Search<'_>) -> DistributionTable {
    sampling::tier_outcomes(tier_weights).map(|outcomes| {
        let mut options: Vec<_> = outcomes
            .into_iter()
            .map(|outcome| {
                let u = search.unit().max(f64::MIN_POSITIVE);
                (outcome.counts, outcome.log_mass - (-u.ln()).ln())
            })
            .collect();
        options.sort_by(|a, b| b.1.total_cmp(&a.1));
        options.into_iter().map(|(counts, _)| counts).collect()
    })
}
