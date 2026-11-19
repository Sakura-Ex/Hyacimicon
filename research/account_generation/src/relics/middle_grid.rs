//! 将遗器候选分数映射到有限精度整数网格，搜索可达总分并还原组合。

use std::collections::BTreeMap;

use crate::{GenerationError, Search};

use super::{Bounds, EPSILON, Group, contains};

/// 带上限的可达整数分数集合；第 n 位表示恰好可取得 n 个网格单位。
struct Scores {
    /// 位图数据，每个 `u64` 按低位优先编码连续 64 个分数。
    words: Vec<u64>,
    /// 当前位图中允许查询的最大非负整数分数索引。
    maximum: usize,
}

/// 维护遗器分数组合的整数可达集合。
impl Scores {
    /// 创建仅包含零分的可达集合。
    fn zero() -> Self {
        Self {
            words: vec![1],
            maximum: 0,
        }
    }

    /// 判断非负整数分数是否在上限内且对应位已置位。
    fn contains(&self, score: usize) -> bool {
        score <= self.maximum && self.words[score / 64] & (1 << (score % 64)) != 0
    }

    /// 将当前可达值与单件选项做和集合并，并截断到 `cap`。
    fn extend(&self, options: &[usize], cap: usize) -> Self {
        let maximum = self
            .maximum
            .saturating_add(options.last().copied().unwrap_or_default())
            .min(cap);
        let mut words = vec![0; maximum / 64 + 1];
        for &option in options {
            let shift = option / 64;
            let bits = option % 64;
            if shift >= words.len() {
                break;
            }
            let length = words.len();
            for (i, &word) in self.words.iter().take(length - shift).enumerate() {
                words[shift + i] |= word << bits;
                if bits > 0 && shift + i + 1 < length {
                    words[shift + i + 1] |= word >> (64 - bits);
                }
            }
        }
        let bits = maximum % 64;
        if bits != 63 {
            words[maximum / 64] &= (1 << (bits + 1)) - 1;
        }
        Self { words, maximum }
    }
}

/// 一个角色在整数网格上的单件分数选项及后缀可达集合。
struct ReachableGroup {
    /// 每件的网格分数到实际分数映射，用于回溯原始浮点值。
    recipes: Vec<BTreeMap<usize, f64>>,
    /// 从每个部位起算的可达分数位图，最后一项表示空后缀只能取零分。
    suffix: Vec<Scores>,
    /// 同时满足该角色区间且可由各件组合得到的整数分数。
    possible: Vec<usize>,
}

/// 将可达中档分数映射到整数网格，按角色及整号目标回溯各件分数。
/// 原始浮点分数通过复核后，每件计一次预算。无法表示或找不到符合区间的组合时返回 `None`，供调用方改用全档位搜索。
pub(super) fn find(
    groups: &[Group],
    overall: Bounds,
    desired: &[f64],
    search: &mut Search<'_>,
) -> Result<Option<Vec<Vec<f64>>>, GenerationError> {
    let mut scales: Vec<_> = [1.0, 10.0, 100.0, 1000.0, 10_000.0]
        .into_iter()
        .filter(|scale| overall.1 * scale <= 50_000.0)
        .collect();
    if scales.is_empty() {
        return Ok(None);
    }
    let mut candidates = Vec::with_capacity(groups.len());
    for group in groups {
        let mut pieces = Vec::with_capacity(group.pieces.len());
        for piece in &group.pieces {
            let mut options = Vec::new();
            for candidate in piece.candidates(piece.bounds, None, search) {
                scales.retain(|&scale| {
                    let score = candidate * scale;
                    score >= 0.0 && (score - score.round()).abs() <= 1e-10
                });
                if scales.is_empty() {
                    return Ok(None);
                }
                options.push(candidate);
            }
            if options.is_empty() {
                return Ok(None);
            }
            pieces.push(options);
        }
        candidates.push(pieces);
    }
    let scale = scales[0];
    let Some((low, high)) = integer_bounds(overall, scale) else {
        return Ok(None);
    };
    let mut reachable = Vec::new();
    for (group, pieces) in groups.iter().zip(candidates) {
        let Some((min, max)) = integer_bounds(group.bounds, scale) else {
            return Ok(None);
        };
        let max = max.min(high);
        let recipes: Vec<BTreeMap<_, _>> = pieces
            .into_iter()
            .map(|pieces| {
                pieces
                    .into_iter()
                    .map(|score| ((score * scale).round() as usize, score))
                    .collect()
            })
            .collect();
        let suffix = suffix(
            recipes
                .iter()
                .map(|options| options.keys().copied().collect()),
            max,
        );
        let possible: Vec<_> = (min..=max.min(suffix[0].maximum))
            .filter(|&score| suffix[0].contains(score))
            .collect();
        if possible.is_empty() {
            return Ok(None);
        }
        reachable.push(ReachableGroup {
            recipes,
            suffix,
            possible,
        });
    }
    let suffix = suffix(reachable.iter().map(|group| group.possible.clone()), high);
    let goal: f64 = desired.iter().sum::<f64>() * scale;
    let Some(mut remaining) = (low..=high.min(suffix[0].maximum))
        .filter(|&score| suffix[0].contains(score))
        .min_by(|&a, &b| (a as f64 - goal).abs().total_cmp(&(b as f64 - goal).abs()))
    else {
        return Ok(None);
    };
    let mut output = Vec::new();
    let mut actual_total = 0.0;
    for (i, (group, options)) in groups.iter().zip(&reachable).enumerate() {
        let goal = desired[i] * scale;
        let Some(mut score) = options
            .possible
            .iter()
            .copied()
            .filter(|&score| {
                remaining
                    .checked_sub(score)
                    .is_some_and(|rest| suffix[i + 1].contains(rest))
            })
            .min_by(|&a, &b| (a as f64 - goal).abs().total_cmp(&(b as f64 - goal).abs()))
        else {
            return Ok(None);
        };
        remaining -= score;
        let mut pieces = Vec::new();
        for j in 0..group.pieces.len() {
            let target = score as f64 / (group.pieces.len() - j) as f64;
            let Some((&key, candidate)) = options.recipes[j]
                .iter()
                .filter(|&(&key, _)| {
                    score
                        .checked_sub(key)
                        .is_some_and(|rest| options.suffix[j + 1].contains(rest))
                })
                .min_by(|&(&a, _), &(&b, _)| {
                    (a as f64 - target)
                        .abs()
                        .total_cmp(&(b as f64 - target).abs())
                })
            else {
                return Ok(None);
            };
            pieces.push(*candidate);
            score -= key;
        }
        let actual_score: f64 = pieces.iter().sum();
        if !contains(group.bounds, actual_score) {
            return Ok(None);
        }
        actual_total += actual_score;
        output.push(pieces);
    }
    // 网格的舍入只用于找组合，预算只记通过原始分数范围的成品。
    if !contains(overall, actual_total) {
        return Ok(None);
    }
    for _ in output.iter().flatten() {
        search.consider()?;
    }
    Ok(Some(output))
}

/// 为每个后缀构造可达和集合，用于分数组合剪枝及回溯。
fn suffix(options: impl DoubleEndedIterator<Item = Vec<usize>>, cap: usize) -> Vec<Scores> {
    let mut result = Vec::new();
    let mut current = Scores::zero();
    for options in options.rev() {
        let next = current.extend(&options, cap);
        result.push(current);
        current = next;
    }
    result.push(current);
    result.reverse();
    result
}

/// 将实数分数区间映射到给定缩放下可接受的非负整数边界。
fn integer_bounds(bounds: Bounds, scale: f64) -> Option<(usize, usize)> {
    let low = ((bounds.0 - EPSILON) * scale).ceil().max(0.0);
    let high = ((bounds.1 + EPSILON) * scale).floor();
    (low <= high).then_some((low as usize, high as usize))
}
