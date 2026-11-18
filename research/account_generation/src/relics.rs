//! 五星 +15 遗器生成，保留合法词条次数及离散计分。

mod candidates;
mod middle_grid;

use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicInstance, RelicMainStat, RelicSlot, RelicSubStat};
use serde::Deserialize;

use crate::{AccountRequest, GenerationError, RngCore, Search, allocation, invalid};
use candidates::{Candidates, PieceSearch};

pub(super) const EPSILON: f64 = 1e-6;
pub(super) const SLOTS: [RelicSlot; 6] = [
    RelicSlot::Head,
    RelicSlot::Hands,
    RelicSlot::Body,
    RelicSlot::Feet,
    RelicSlot::PlanarSphere,
    RelicSlot::LinkRope,
];
const QUALITY_RATIOS: [f64; 6] = [0.0, 0.35, 0.5, 0.65, 0.8, 1.0];

/// 一个部位的配装条件，留空字段采用该部位的合法候选。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicSpec {
    pub slot: RelicSlot,
    #[serde(default)]
    pub set_id: Option<String>,
    #[serde(default)]
    pub mainstat: Vec<RelicMainStat>,
    #[serde(default)]
    pub initial_substat_count: Option<u8>,
    #[serde(default)]
    pub required_substats: Vec<RelicSubStat>,
    #[serde(default)]
    pub min_effective_rolls: Option<f64>,
    #[serde(default)]
    pub max_effective_rolls: Option<f64>,
}

impl RelicSpec {
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
    pub pieces: Vec<RelicSpec>,
    pub effective_stat_weights: BTreeMap<RelicSubStat, f64>,
    pub tier_weights: [f64; 3],
    pub min_total_effective_rolls: Option<f64>,
    pub max_total_effective_rolls: Option<f64>,
}

impl Default for RelicRequest {
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

impl RelicRequest {
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
            .any(|&weight| !weight.is_finite() || weight.abs() > f64::MAX / 128.0)
        {
            return Err(invalid("副词条评分权重需要为可计算的有限数"));
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

pub(super) struct Gear {
    pub pieces: Vec<(RelicInstance, f64)>,
    pub upper: f64,
}

/// 使用请求对应的中档数值计算有效词条数。
pub fn score_relic(relic: &RelicInstance, weights: &BTreeMap<RelicSubStat, f64>) -> f64 {
    relic
        .substats
        .iter()
        .map(|(&stat, &value)| {
            weights.get(&stat).copied().unwrap_or_default() * (value / candidates::rolls(stat)[1])
        })
        .sum()
}

/// 独立生成六件遗器，缺少的部位用合法候选补齐。
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

/// 计算六件的理论有效词条上限，依据配装条件和允许的档位。
pub fn maximum_character_relic_score(request: &RelicRequest) -> Result<f64, GenerationError> {
    request.validate()?;
    complete_specs(request)
        .iter()
        .map(|spec| candidates::maximum_score(spec, request))
        .sum()
}

fn complete_specs(request: &RelicRequest) -> Vec<RelicSpec> {
    let mut specs = request.pieces.clone();
    for slot in SLOTS {
        if !specs.iter().any(|piece| piece.slot == slot) {
            specs.push(RelicSpec::new(slot));
        }
    }
    specs
}

/// 独立生成指定部位的一件遗器。
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
    let mut groups = Vec::new();
    for choice in choices {
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
    // 总分及上下界之差都参与分配，包括有正负权重的账号。
    let magnitude: f64 = bounds.iter().map(|&(lo, hi)| lo.abs().max(hi.abs())).sum();
    if !magnitude.is_finite()
        || !bounds
            .iter()
            .map(|&(lo, hi)| hi - lo)
            .sum::<f64>()
            .is_finite()
    {
        return Err(invalid("账号遗器总分超出可计算范围"));
    }
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
    if (strength == 0.0 || groups.len() == 1) && overall == sum_bounds(bounds.iter().copied()) {
        return groups
            .iter()
            .map(|group| sample_group(group, search))
            .collect();
    }
    let weights = allocation::weights(
        choices.iter().map(|choice| choice.profile.id.as_str()),
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
    )
}

struct Group {
    pieces: Vec<PieceSearch>,
    suffix: Vec<Bounds>,
    bounds: Bounds,
    upper: f64,
}

impl Group {
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

// 分配器给出一组同时达标的分数。依次替换各件时，保留其他件已占用的分数，按真实允许档位抽取成品。
fn sample_allocated_scores(
    groups: &[Group],
    scores: Vec<Vec<f64>>,
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
        let bounds = intersect(
            piece.bounds,
            intersect(
                (group.bounds.0 - group_rest, group.bounds.1 - group_rest),
                (overall.0 - rest, overall.1 - rest),
            ),
        );
        // 集中投入时保留分配目标；精确总分不把舍入容差逐件累积。
        let bounds = if preserve_allocation || bounds.1 - bounds.0 <= 2.0 * EPSILON + 1e-12 {
            intersect(piece.bounds, (planned, planned))
        } else {
            bounds
        };
        let candidate = piece.sample(bounds, search)?;
        totals[group_idx] = group_rest + candidate.score;
        total = rest + candidate.score;
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

struct GearSearch<'a, 'b, 'rng> {
    groups: &'a [Group],
    overall: Bounds,
    desired: &'a [f64],
    suffix: &'a [Bounds],
    search: &'b mut Search<'rng>,
    output: Vec<Vec<f64>>,
    failed: BTreeSet<(usize, usize, u64, u64)>,
}

#[derive(Clone, Copy, Default)]
struct Position {
    group: usize,
    piece: usize,
    earlier_score: f64,
    group_score: f64,
}

impl Position {
    fn key(self) -> (usize, usize, u64, u64) {
        (
            self.group,
            self.piece,
            self.earlier_score.to_bits(),
            self.group_score.to_bits(),
        )
    }
}

struct Frame<'a> {
    position: Position,
    group_bounds: Bounds,
    candidates: Candidates<'a>,
}

impl<'a> GearSearch<'a, '_, '_> {
    fn frame(&mut self, position: Position) -> Option<Frame<'a>> {
        if self.failed.contains(&position.key()) {
            return None;
        }
        let group = &self.groups[position.group];
        let tail = self.suffix[position.group + 1];
        let group_bounds = intersect(
            group.bounds,
            (
                self.overall.0 - position.earlier_score - tail.1,
                self.overall.1 - position.earlier_score - tail.0,
            ),
        );
        let piece = &group.pieces[position.piece];
        let tail = group.suffix[position.piece + 1];
        let local = intersect(
            piece.bounds,
            (
                group_bounds.0 - position.group_score - tail.1,
                group_bounds.1 - position.group_score - tail.0,
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

type Bounds = (f64, f64);

fn sum_bounds(values: impl Iterator<Item = Bounds>) -> Bounds {
    values.fold((0.0, 0.0), |(lo, hi), (min, max)| (lo + min, hi + max))
}

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

fn score_bounds(min: Option<f64>, max: Option<f64>) -> Bounds {
    (
        min.map_or(f64::NEG_INFINITY, |value| value - EPSILON),
        max.map_or(f64::INFINITY, |value| value + EPSILON),
    )
}

fn intersect(a: Bounds, b: Bounds) -> Bounds {
    (a.0.max(b.0), a.1.min(b.1))
}

fn contains(bounds: Bounds, score: f64) -> bool {
    let rounding = (8.0 * f64::EPSILON * score.abs()).min(1e-12);
    score >= bounds.0 - rounding && score <= bounds.1 + rounding
}

fn feasible(bounds: Bounds) -> Result<(), GenerationError> {
    if bounds.0 > bounds.1 + EPSILON {
        Err(GenerationError::NoSolution)
    } else {
        Ok(())
    }
}

fn quality_bounds(tier: u8, upper: f64) -> Result<Bounds, GenerationError> {
    if !upper.is_finite() {
        return Err(invalid("遗器理论总分超出可计算范围"));
    }
    if upper <= 0.0 {
        return Err(GenerationError::NoSolution);
    }
    let index = usize::from(tier);
    let tolerance = EPSILON.min(upper * 1e-8);
    let adjustment = if index < 5 { tolerance } else { 0.0 };
    Ok((
        QUALITY_RATIOS[index - 1] * upper - tolerance,
        QUALITY_RATIOS[index] * upper - adjustment,
    ))
}

pub(super) fn validate_quality(quality: Option<u8>) -> Result<(), GenerationError> {
    if quality.is_some_and(|tier| !(1..=5).contains(&tier)) {
        Err(invalid("遗器练度档位取 1～5"))
    } else {
        Ok(())
    }
}

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

fn set_range(slot: RelicSlot) -> std::ops::RangeInclusive<u16> {
    if u8::from(slot) <= 4 {
        101..=134
    } else {
        301..=330
    }
}

pub(super) fn supports_set(slot: RelicSlot, id: &str) -> bool {
    id.parse::<u16>()
        .is_ok_and(|set| set_range(slot).contains(&set) && set.to_string() == id)
}
