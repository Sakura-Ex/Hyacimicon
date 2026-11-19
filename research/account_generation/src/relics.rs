//! 五星 +15 遗器生成，保留合法词条次数及离散计分。

mod candidates;
mod middle_grid;

use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicInstance, RelicMainStat, RelicSlot, RelicSubStat};
use serde::Deserialize;

use crate::{AccountRequest, GenerationError, RngCore, Search, allocation, invalid};
use candidates::{Candidates, PieceSearch};

/// 分数区间比较和约束放宽使用的绝对容差，单位为有效词条分。
pub(super) const EPSILON: f64 = 1e-6;
/// 遗器请求和账号统计共用的部位次序。
pub(super) const SLOTS: [RelicSlot; 6] = [
    RelicSlot::Head,
    RelicSlot::Hands,
    RelicSlot::Body,
    RelicSlot::Feet,
    RelicSlot::PlanarSphere,
    RelicSlot::LinkRope,
];
/// 相邻两项给出一个练度档位的下、上边界占比，第一档从零分开始。
const QUALITY_RATIOS: [f64; 6] = [0.0, 0.35, 0.5, 0.65, 0.8, 1.0];
/// 自定义有效词条权重允许的最大绝对值。
const MAX_STAT_WEIGHT: f64 = 100.0;

/// 一个部位的配装条件，留空字段采用该部位的合法候选。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicSpec {
    /// 遗器部位。
    pub slot: RelicSlot,
    /// 套装 ID；留空时从该部位支持的套装中选择。
    #[serde(default)]
    pub set_id: Option<String>,
    /// 主词条候选；留空时使用该部位的全部合法主词条。
    #[serde(default)]
    pub mainstat: Vec<RelicMainStat>,
    /// 初始副词条数可取 3 或 4。留空时两者的抽样权重为 5 比 2，分数约束会改变成品比例。
    #[serde(default)]
    pub initial_substat_count: Option<u8>,
    /// 成品必须包含的副词条，最多四种且不能重复。
    #[serde(default)]
    pub required_substats: Vec<RelicSubStat>,
    /// 单件有效词条数下限。
    #[serde(default)]
    pub min_effective_rolls: Option<f64>,
    /// 单件有效词条数上限。
    #[serde(default)]
    pub max_effective_rolls: Option<f64>,
}

/// 提供遗器部位条件的构造与校验。
impl RelicSpec {
    /// 创建指定部位的默认请求，候选条件均留空。
    pub fn new(slot: RelicSlot) -> Self {
        Self {
            slot,
            set_id: None,
            mainstat: Vec::new(),
            initial_substat_count: None,
            required_substats: Vec::new(),
            min_effective_rolls: None,
            max_effective_rolls: None,
        }
    }

    /// 检查套装、部位、主词条、副词条和单件分数范围是否合法。
    fn validate(&self) -> Result<(), GenerationError> {
        if self
            .set_id
            .as_deref()
            .is_some_and(|id| !supports_set(self.slot, id))
        {
            return Err(invalid(format!(
                "套装与部位 {} 不匹配",
                u8::from(self.slot)
            )));
        }
        if self
            .mainstat
            .iter()
            .any(|stat| !stat.supports_slot(self.slot))
        {
            return Err(invalid("主词条与部位不匹配"));
        }
        if self
            .initial_substat_count
            .is_some_and(|count| !matches!(count, 3 | 4))
        {
            return Err(invalid("初始副词条数取 3 或 4"));
        }
        if self.required_substats.len() > RelicInstance::MAX_SUBSTATS
            || self.required_substats.iter().collect::<BTreeSet<_>>().len()
                != self.required_substats.len()
        {
            return Err(invalid("必含副词条需要互异，且最多四种"));
        }
        validate_range(self.min_effective_rolls, self.max_effective_rolls)
    }
}

/// 一组遗器的覆盖、计分、数值档位和整体分数条件。
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RelicRequest {
    /// 显式覆盖的部位；未列出的部位由生成器补齐。
    pub pieces: Vec<RelicSpec>,
    /// 每次中档副词条的计分权重，取 -100 至 100。未列出的属性计零分。
    pub effective_stat_weights: BTreeMap<RelicSubStat, f64>,
    /// 低、中、高三档的相对抽样权重。每项须为有限非负数，至少一项为正。零权重的档位不参与生成。
    pub tier_weights: [f64; 3],
    /// 六件遗器有效词条总数下限。
    pub min_total_effective_rolls: Option<f64>,
    /// 六件遗器有效词条总数上限。
    pub max_total_effective_rolls: Option<f64>,
}

/// 为遗器请求提供空部位、等档位权重和无分数约束的默认值。
impl Default for RelicRequest {
    /// 创建无显式部位条件、三档权重相等且无分数限制的请求。
    fn default() -> Self {
        Self {
            pieces: Vec::new(),
            effective_stat_weights: BTreeMap::new(),
            tier_weights: [1.0; 3],
            min_total_effective_rolls: None,
            max_total_effective_rolls: None,
        }
    }
}

/// 验证遗器组请求及其跨部位计分参数。
impl RelicRequest {
    /// 检查部位唯一性、分数范围、评分权重和档位权重。
    pub(crate) fn validate(&self) -> Result<(), GenerationError> {
        let mut slots = BTreeSet::new();
        for piece in &self.pieces {
            piece.validate()?;
            if !slots.insert(piece.slot) {
                return Err(invalid("遗器请求中的部位重复"));
            }
        }
        validate_range(
            self.min_total_effective_rolls,
            self.max_total_effective_rolls,
        )?;
        if self
            .effective_stat_weights
            .values()
            .any(|weight| !(-MAX_STAT_WEIGHT..=MAX_STAT_WEIGHT).contains(weight))
        {
            return Err(invalid("副词条评分权重须在 -100 至 100 之间"));
        }
        if self
            .tier_weights
            .iter()
            .any(|&weight| !weight.is_finite() || weight < 0.0)
            || self.tier_weights.iter().all(|&weight| weight == 0.0)
        {
            return Err(invalid("副词条档位权重需要非负，且至少一档为正"));
        }
        Ok(())
    }
}

/// 一组已生成的遗器、各件实际得分及本组理论上限。
pub(super) struct Gear {
    /// 按本组输入部位顺序排列的遗器及其有效词条分数。
    pub pieces: Vec<(RelicInstance, f64)>,
    /// 当前角色配装约束下的理论最高分。
    pub upper: f64,
}

/// 将成品每条副词条的数值除以该属性的单次中档值，再乘以调用方给出的权重并求和。
/// 未指定权重的属性计零分。百分比使用原始比例值，例如 6.48% 暴击伤害传入 0.0648。
pub fn score_relic(relic: &RelicInstance, weights: &BTreeMap<RelicSubStat, f64>) -> f64 {
    relic
        .substats
        .iter()
        .map(|(&stat, &value)| {
            weights.get(&stat).copied().unwrap_or_default() * (value / candidates::rolls(stat)[1])
        })
        .sum()
}

/// 按请求生成六件五星 +15 遗器，返回各件实例及实际得分。显式部位保留输入顺序，缺少的部位按标准部位顺序追加。
/// 搜索共用 `max_attempts`。确认分数目标不可达时返回 [`GenerationError::NoSolution`]，
/// 用尽预算后仍需考察候选时返回 [`GenerationError::BudgetExhausted`]。
pub fn generate_character_relics(
    request: &RelicRequest,
    max_attempts: usize,
    rng: &mut dyn RngCore,
) -> Result<Vec<(RelicInstance, f64)>, GenerationError> {
    request.validate()?;
    let mut search = Search {
        rng,
        attempts: 0,
        limit: max_attempts,
    };
    generate_set(request, &complete_specs(request), &mut search)
}

/// 计算六件合法成品在配装条件和允许档位下的理论最高分，不应用单件或总分目标区间。
pub fn maximum_character_relic_score(request: &RelicRequest) -> Result<f64, GenerationError> {
    request.validate()?;
    complete_specs(request)
        .iter()
        .map(|spec| candidates::maximum_score(spec, request))
        .sum()
}

/// 保留显式部位顺序，并按头、手、躯干、脚、球、绳顺序追加缺失部位。
fn complete_specs(request: &RelicRequest) -> Vec<RelicSpec> {
    let mut specs = request.pieces.clone();
    for slot in SLOTS {
        if !specs.iter().any(|piece| piece.slot == slot) {
            specs.push(RelicSpec::new(slot));
        }
    }
    specs
}

/// 在指定部位、主副词条条件和单件得分区间内抽取一件五星 +15 遗器，同时返回成品得分。
/// `weights` 按中档值计分，`tier_weights` 指定低、中、高档的相对概率。成品的实例 ID 留空，由装配方分配。
pub fn generate_relic(
    spec: &RelicSpec,
    weights: &BTreeMap<RelicSubStat, f64>,
    tier_weights: [f64; 3],
    max_attempts: usize,
    rng: &mut dyn RngCore,
) -> Result<(RelicInstance, f64), GenerationError> {
    let request = RelicRequest {
        pieces: vec![spec.clone()],
        effective_stat_weights: weights.clone(),
        tier_weights,
        ..RelicRequest::default()
    };
    request.validate()?;
    let mut search = Search {
        rng,
        attempts: 0,
        limit: max_attempts,
    };
    generate_set(&request, &request.pieces, &mut search)?
        .pop()
        .ok_or(GenerationError::NoSolution)
}

/// 为给定部位条件建立搜索状态并抽取一整组遗器。
fn generate_set(
    request: &RelicRequest,
    specs: &[RelicSpec],
    search: &mut Search<'_>,
) -> Result<Vec<(RelicInstance, f64)>, GenerationError> {
    let pieces = specs
        .iter()
        .map(|spec| PieceSearch::new(spec, request, search))
        .collect::<Result<Vec<_>, _>>()?;
    let group = Group::new(pieces, request, None)?;
    Ok(sample_group(&group, search)?.pieces)
}

/// 按推荐配装和账号级分数目标生成每个角色的遗器组。
pub(super) fn generate(
    request: &AccountRequest,
    choices: &[allocation::Choice<'_>],
    cores: &[String],
    search: &mut Search<'_>,
) -> Result<Vec<Gear>, GenerationError> {
    if choices.is_empty() {
        if request.min_mean_effective_rolls.unwrap_or_default() > 0.0
            || request.max_mean_effective_rolls.unwrap_or_default() < 0.0
            || request.relic_quality_tier.is_some()
        {
            return Err(GenerationError::NoSolution);
        }
        return Ok(Vec::new());
    }
    // 与 GeneratedAccount::investments 使用相同的角色 ID 顺序累计总分和上限。
    let mut order: Vec<_> = (0..choices.len()).collect();
    order.sort_unstable_by_key(|&index| &choices[index].profile.id);
    let mut groups = Vec::with_capacity(choices.len());
    for &index in &order {
        let choice = &choices[index];
        let recommended = &choice.profile.relics;
        let outer = &recommended.outer[search.index(recommended.outer.len())];
        let planar = &recommended.planar[search.index(recommended.planar.len())];
        let defaults;
        let conditions = match &choice.spec.relics {
            Some(request) => request,
            None => {
                defaults = RelicRequest {
                    effective_stat_weights: recommended.weights.clone(),
                    ..RelicRequest::default()
                };
                &defaults
            }
        };
        let pieces = SLOTS
            .into_iter()
            .zip(&recommended.mains)
            .map(|(slot, mains)| {
                if let Some(spec) = conditions.pieces.iter().find(|piece| piece.slot == slot) {
                    return PieceSearch::new(spec, conditions, search);
                }
                let suggested = RelicSpec {
                    set_id: Some(if u8::from(slot) <= 4 { outer } else { planar }.clone()),
                    mainstat: mains.clone(),
                    ..RelicSpec::new(slot)
                };
                PieceSearch::new(&suggested, conditions, search)
            })
            .collect::<Result<Vec<_>, _>>()?;
        groups.push(Group::new(
            pieces,
            conditions,
            choice.spec.relic_quality_tier,
        )?);
    }
    let bounds: Vec<_> = groups.iter().map(|group| group.bounds).collect();
    let mut overall = sum_bounds(bounds.iter().copied());
    let count = (choices.len() * SLOTS.len()) as f64;
    overall = intersect(
        overall,
        score_bounds(
            request.min_mean_effective_rolls.map(|v| v * count),
            request.max_mean_effective_rolls.map(|v| v * count),
        ),
    );
    if let Some(quality) = request.relic_quality_tier {
        overall = intersect(
            overall,
            quality_bounds(quality, groups.iter().map(|group| group.upper).sum())?,
        );
    }
    feasible(overall)?;
    let strength = request
        .relic_concentration
        .unwrap_or(request.resource_concentration);
    let gear = if (strength == 0.0 || groups.len() == 1)
        && overall == sum_bounds(bounds.iter().copied())
    {
        groups
            .iter()
            .map(|group| sample_group(group, search))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let weights = allocation::weights(
            order
                .iter()
                .map(|&index| choices[index].profile.id.as_str()),
            cores,
            strength,
        );
        let desired = allocation::distribute(
            overall.0 + search.unit() * (overall.1 - overall.0),
            &bounds,
            &weights,
        );
        solve(
            &groups,
            overall,
            &desired,
            strength > 0.0 && groups.len() > 1,
            search,
        )?
    };
    // 装备按原名单顺序交还给账号组装。
    let mut loadouts: Vec<_> = order.into_iter().zip(gear).collect();
    loadouts.sort_unstable_by_key(|(index, _)| *index);
    Ok(loadouts.into_iter().map(|(_, gear)| gear).collect())
}

/// 一组遗器候选及其可行总分区间；角色账号生成时包含六个部位。
struct Group {
    /// 按输入条件排列的单件搜索器；账号生成会按头、手、躯干、脚、球、绳提供。
    pieces: Vec<PieceSearch>,
    /// 从各位置起算的剩余可达分数区间，末项为零分。
    suffix: Vec<Bounds>,
    /// 单件条件、角色总分和练度档共同裁定的组分区间。
    bounds: Bounds,
    /// 各部位理论最高分之和，用于换算练度档位。
    upper: f64,
}

/// 汇总本组各件的分数区间，再与请求的总分及练度档位取交集。
impl Group {
    /// 构造可行的遗器组；单件或六件入口均可调用，约束无交集时返回 `NoSolution`。
    fn new(
        pieces: Vec<PieceSearch>,
        request: &RelicRequest,
        quality: Option<u8>,
    ) -> Result<Self, GenerationError> {
        let upper = pieces.iter().map(|piece| piece.upper).sum();
        let suffix = suffix_bounds(pieces.iter().map(|piece| piece.bounds));
        let mut bounds = intersect(
            suffix[0],
            score_bounds(
                request.min_total_effective_rolls,
                request.max_total_effective_rolls,
            ),
        );
        if let Some(quality) = quality {
            bounds = intersect(bounds, quality_bounds(quality, upper)?);
        }
        feasible(bounds)?;
        Ok(Self {
            pieces,
            suffix,
            bounds,
            upper,
        })
    }
}

/// 抽取满足角色组分数范围的遗器；范围未收窄时逐件独立抽样。
fn sample_group(group: &Group, search: &mut Search<'_>) -> Result<Gear, GenerationError> {
    if group.bounds == group.suffix[0] {
        let mut pieces = Vec::with_capacity(group.pieces.len());
        for piece in &group.pieces {
            let candidate = piece.sample(piece.bounds, search)?;
            search.consider()?;
            pieces.push((piece.materialize(candidate), candidate.score));
        }
        return Ok(Gear {
            pieces,
            upper: group.upper,
        });
    }
    let (lo, hi) = group.bounds;
    let desired = lo + search.unit() * (hi - lo);
    solve(
        std::slice::from_ref(group),
        group.bounds,
        &[desired],
        false,
        search,
    )?
    .pop()
    .ok_or(GenerationError::NoSolution)
}

/// 搜索同时满足角色组与账号总分范围的单件分数组合。
fn solve(
    groups: &[Group],
    overall: Bounds,
    desired: &[f64],
    preserve_allocation: bool,
    search: &mut Search<'_>,
) -> Result<Vec<Gear>, GenerationError> {
    let can_use_middle = groups
        .iter()
        .flat_map(|group| &group.pieces)
        .all(PieceSearch::has_middle);
    let count: usize = groups.iter().map(|group| group.pieces.len()).sum();
    if can_use_middle
        && search.limit - search.attempts >= count
        && let Some(scores) = middle_grid::find(groups, overall, desired, search)?
    {
        return sample_allocated_scores(groups, scores, overall, preserve_allocation, search);
    }
    let suffix = suffix_bounds(groups.iter().map(|group| group.bounds));
    let mut solver = GearSearch {
        groups,
        overall,
        desired,
        suffix: &suffix,
        search,
        output: vec![Vec::new(); groups.len()],
        failed: BTreeSet::new(),
    };
    if solver.solve()? {
        sample_allocated_scores(groups, solver.output, overall, preserve_allocation, search)
    } else {
        Err(GenerationError::NoSolution)
    }
}

/// 在保留其他件已占分数的条件下，按允许的强化档位逐件抽取成品，最后复核角色及账号总分。
fn sample_allocated_scores(
    groups: &[Group],
    mut scores: Vec<Vec<f64>>,
    overall: Bounds,
    preserve_allocation: bool,
    search: &mut Search<'_>,
) -> Result<Vec<Gear>, GenerationError> {
    let mut totals: Vec<f64> = scores.iter().map(|group| group.iter().sum()).collect();
    let mut total: f64 = totals.iter().sum();
    let mut order: Vec<_> = groups
        .iter()
        .enumerate()
        .flat_map(|(group_idx, group)| (0..group.pieces.len()).map(move |slot| (group_idx, slot)))
        .collect();
    search.shuffle(&mut order);
    let mut output = vec![Vec::new(); groups.len()];
    for (group_idx, slot) in order {
        let group = &groups[group_idx];
        let piece = &group.pieces[slot];
        let planned = scores[group_idx][slot];
        let group_rest = totals[group_idx] - planned;
        let rest = total - planned;
        let (lo, hi) = intersect(
            piece.bounds,
            intersect(
                (group.bounds.0 - group_rest, group.bounds.1 - group_rest),
                (overall.0 - rest, overall.1 - rest),
            ),
        );
        // 分配器已验证 planned，减去其他部位的得分时可能产生舍入误差。
        let bounds = (lo.min(planned), hi.max(planned));
        // 集中投入或精确总分保留已分配的目标，避免将整号容差用于逐件调分。
        let bounds = if preserve_allocation || bounds.1 - bounds.0 <= 2.0 * EPSILON + 1e-12 {
            intersect(bounds, with_roundoff((planned, planned)))
        } else {
            bounds
        };
        let candidate = piece.sample(bounds, search)?;
        // 按输出顺序合计；若替换跨过档位边界，保留原分数的成品。
        let previous_total = totals[group_idx];
        scores[group_idx][slot] = candidate.score;
        totals[group_idx] = scores[group_idx].iter().sum();
        let new_total = totals.iter().sum();
        let candidate = if contains(group.bounds, totals[group_idx]) && contains(overall, new_total)
        {
            total = new_total;
            candidate
        } else {
            scores[group_idx][slot] = planned;
            totals[group_idx] = previous_total;
            piece.sample((planned, planned), search)?
        };
        output[group_idx].push((slot, (piece.materialize(candidate), candidate.score)));
    }
    Ok(groups
        .iter()
        .zip(output)
        .map(|(group, mut choices)| {
            choices.sort_unstable_by_key(|(slot, _)| *slot);
            Gear {
                upper: group.upper,
                pieces: choices.into_iter().map(|(_, piece)| piece).collect(),
            }
        })
        .collect())
}

/// 使用显式栈和失败状态缓存执行遗器分数组合深度优先搜索。
struct GearSearch<'a, 'b, 'rng> {
    /// 参与搜索的角色组，顺序与账号计分保持一致。
    groups: &'a [Group],
    /// 所有角色合计必须落入的有效词条分区间。
    overall: Bounds,
    /// 每个角色的目标分数，用于优先探索邻近候选。
    desired: &'a [f64],
    /// 从各角色位置起算的剩余分数可达范围。
    suffix: &'a [Bounds],
    /// 共享随机源与候选考察预算。
    search: &'b mut Search<'rng>,
    /// 当前路径已选的单件分数，每个角色对应一个列表。
    output: Vec<Vec<f64>>,
    /// 已证明无解的搜索位置键，避免重复展开。
    failed: BTreeSet<(usize, usize, u64, u64)>,
}

/// 记录角色组、部位及此前已累计分数的 DFS 位置。
#[derive(Clone, Copy, Default)]
struct Position {
    /// 当前角色组在 `GearSearch::groups` 中的索引。
    group: usize,
    /// 当前角色组内遗器部位索引。
    piece: usize,
    /// 当前角色之前各组的累计分数。
    earlier_score: f64,
    /// 当前角色组内已选遗器的累计分数。
    group_score: f64,
}

/// 将 DFS 位置转换为失败缓存使用的精确浮点位模式键。
impl Position {
    /// 返回包含组索引、部位索引和累计分数位模式的缓存键。
    fn key(self) -> (usize, usize, u64, u64) {
        (
            self.group,
            self.piece,
            self.earlier_score.to_bits(),
            self.group_score.to_bits(),
        )
    }
}

/// 保存一个 DFS 搜索层的位置、组约束和待枚举候选。
struct Frame<'a> {
    /// 当前组和部位，以及抵达该层时的累计分数。
    position: Position,
    /// 扣除其他角色后当前角色仍可使用的总分范围。
    group_bounds: Bounds,
    /// 当前遗器部位按分数距离排列的候选迭代器。
    candidates: Candidates<'a>,
}

/// 用栈保存待检查的部位和候选迭代器，根据后缀分数范围剪枝。
impl<'a> GearSearch<'a, '_, '_> {
    /// 若位置尚未判定失败，生成其角色区间和单件候选帧。
    fn frame(&mut self, position: Position) -> Option<Frame<'a>> {
        if self.failed.contains(&position.key()) {
            return None;
        }
        let group = &self.groups[position.group];
        let group_bounds = intersect(
            group.bounds,
            remaining_bounds(
                self.overall,
                position.earlier_score,
                self.suffix[position.group + 1],
            ),
        );
        let piece = &group.pieces[position.piece];
        let local = intersect(
            piece.bounds,
            remaining_bounds(
                group_bounds,
                position.group_score,
                group.suffix[position.piece + 1],
            ),
        );
        if group_bounds.0 > group_bounds.1 + EPSILON || local.0 > local.1 + EPSILON {
            return None;
        }
        let desired = self.desired[position.group]
            .max(group_bounds.0)
            .min(group_bounds.1);
        let wanted =
            (desired - position.group_score) / (group.pieces.len() - position.piece) as f64;
        Some(Frame {
            position,
            group_bounds,
            candidates: piece.candidates(local, Some(wanted), self.search),
        })
    }

    /// 深度优先枚举候选，找到满足全部区间的分数组合时返回 `true`。
    fn solve(&mut self) -> Result<bool, GenerationError> {
        let Some(first) = self.frame(Position::default()) else {
            return Ok(false);
        };
        let mut stack = vec![first];
        while let Some(frame) = stack.last_mut() {
            let position = frame.position;
            let Some(candidate) = frame.candidates.next() else {
                self.failed.insert(position.key());
                stack.pop();
                if let Some(parent) = stack.last() {
                    self.output[parent.position.group].pop();
                }
                continue;
            };

            self.search.consider()?;
            let group_score = position.group_score + candidate;
            let next = if position.piece + 1 == self.groups[position.group].pieces.len() {
                if !contains(frame.group_bounds, group_score) {
                    continue;
                }
                Position {
                    group: position.group + 1,
                    piece: 0,
                    earlier_score: position.earlier_score + group_score,
                    group_score: 0.0,
                }
            } else {
                Position {
                    piece: position.piece + 1,
                    group_score,
                    ..position
                }
            };

            self.output[position.group].push(candidate);
            if next.group == self.groups.len() {
                if contains(self.overall, next.earlier_score) {
                    return Ok(true);
                }
            } else if let Some(frame) = self.frame(next) {
                stack.push(frame);
                continue;
            }
            self.output[position.group].pop();
        }
        Ok(false)
    }
}

/// 有效词条分的闭区间 `(下限, 上限)`。
type Bounds = (f64, f64);

/// 累加各段下限和上限，得到合计分数的范围。
fn sum_bounds(values: impl Iterator<Item = Bounds>) -> Bounds {
    values.fold((0.0, 0.0), |(lo, hi), (min, max)| (lo + min, hi + max))
}

/// 为每个起点预计算后缀对象可贡献的最小值与最大值之和。
fn suffix_bounds(
    values: impl DoubleEndedIterator<Item = Bounds> + ExactSizeIterator,
) -> Vec<Bounds> {
    let mut bounds = vec![(0.0, 0.0); values.len() + 1];
    for (index, (min, max)) in values.enumerate().rev() {
        let (lo, hi) = bounds[index + 1];
        bounds[index] = (min + lo, max + hi);
    }
    bounds
}

/// 将可选的显式分数上下限转换为含比较容差的区间。
fn score_bounds(min: Option<f64>, max: Option<f64>) -> Bounds {
    (
        min.map_or(f64::NEG_INFINITY, |value| value - EPSILON),
        max.map_or(f64::INFINITY, |value| value + EPSILON),
    )
}

/// 返回两个闭分数区间的交集，即使结果为空也保留交叉端点。
fn intersect(a: Bounds, b: Bounds) -> Bounds {
    (a.0.max(b.0), a.1.min(b.1))
}

/// 扣除已用分和后缀可达分数，再按 `EPSILON` 放宽区间。回溯完成时用原始区间核验总分。
fn remaining_bounds(bounds: Bounds, used: f64, tail: Bounds) -> Bounds {
    (
        bounds.0 - used - tail.1 - EPSILON,
        bounds.1 - used - tail.0 + EPSILON,
    )
}

/// 判断分数是否位于闭区间内。
fn contains(bounds: Bounds, score: f64) -> bool {
    (bounds.0..=bounds.1).contains(&score)
}

/// 按端点绝对值放宽若干浮点刻度，保留按不同顺序累加同一组词条得到的边界分数。
fn with_roundoff((lo, hi): Bounds) -> Bounds {
    (
        lo - 8.0 * f64::EPSILON * lo.abs(),
        hi + 8.0 * f64::EPSILON * hi.abs(),
    )
}

/// 检查区间是否在绝对容差内非空，否则返回无解错误。
fn feasible(bounds: Bounds) -> Result<(), GenerationError> {
    if bounds.0 > bounds.1 + EPSILON {
        Err(GenerationError::NoSolution)
    } else {
        Ok(())
    }
}

/// 按正的理论上限换算档位。相邻档位的分界点归入较高一档，第五档的最高分由合法成品决定。
fn quality_bounds(tier: u8, upper: f64) -> Result<Bounds, GenerationError> {
    if upper <= 0.0 {
        return Err(GenerationError::NoSolution);
    }
    let index = usize::from(tier);
    let end = QUALITY_RATIOS[index] * upper;
    Ok((
        QUALITY_RATIOS[index - 1] * upper,
        // 第五档的上限已由各部位的合法成品限制。
        if tier == 5 {
            f64::INFINITY
        } else {
            end.next_down()
        },
    ))
}

/// 校验可选的练度档位编号是否处于 1 到 5。
pub(super) fn validate_quality(quality: Option<u8>) -> Result<(), GenerationError> {
    if quality.is_some_and(|tier| !(1..=5).contains(&tier)) {
        Err(invalid("遗器练度档位取 1～5"))
    } else {
        Ok(())
    }
}

/// 校验可选分数端点均为有限数，且同时给出时下限不大于上限。
pub(super) fn validate_range(min: Option<f64>, max: Option<f64>) -> Result<(), GenerationError> {
    if [min, max]
        .into_iter()
        .flatten()
        .any(|score| !score.is_finite())
        || min.zip(max).is_some_and(|(min, max)| min > max)
    {
        return Err(invalid("有效词条区间需要有限数值且上下限有序"));
    }
    Ok(())
}

/// 返回部位允许的套装 ID 数值范围：外圈为 101–134，内圈为 301–330。
fn set_range(slot: RelicSlot) -> std::ops::RangeInclusive<u16> {
    if u8::from(slot) <= 4 {
        101..=134
    } else {
        301..=330
    }
}

/// 判断套装 ID 是否为规范十进制形式且属于指定部位的套装范围。
pub(super) fn supports_set(slot: RelicSlot, id: &str) -> bool {
    id.parse::<u16>()
        .is_ok_and(|set| set_range(slot).contains(&set) && set.to_string() == id)
}
