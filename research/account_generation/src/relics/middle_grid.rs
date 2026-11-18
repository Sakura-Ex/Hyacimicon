use std::collections::BTreeMap;

use crate::{GenerationError, Search};

use super::{Bounds, EPSILON, Group, candidates::Candidate};

/// 带上限的可达分数集合。第 n 位表示可取得 n 个整数分数单位。
struct Scores {
    words: Vec<u64>,
    maximum: usize,
}

impl Scores {
    fn zero() -> Self {
        Self {
            words: vec![1],
            maximum: 0,
        }
    }

    fn contains(&self, score: usize) -> bool {
        score <= self.maximum && self.words[score / 64] & (1 << (score % 64)) != 0
    }

    fn extend(&self, options: &[usize], cap: usize) -> Self {
        let maximum = cap.min(self.maximum + options.last().copied().unwrap_or_default());
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

struct ReachableGroup {
    recipes: Vec<BTreeMap<usize, Candidate>>,
    suffix: Vec<Scores>,
    possible: Vec<usize>,
}

/// 先拼合每件及每个角色的可达分数，再逐件还原，窄区间也只需一次成品考察。
pub(super) fn find(
    groups: &[Group],
    overall: Bounds,
    desired: &[f64],
    search: &mut Search<'_>,
) -> Result<Option<Vec<Vec<Candidate>>>, GenerationError> {
    let candidates = groups
        .iter()
        .map(|group| {
            group
                .pieces
                .iter()
                .map(|piece| {
                    let distributions = piece.distributions(true, search);
                    piece.middle_candidates(piece.bounds, 0.0, &distributions)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let Some(scale) = [1.0, 10.0, 100.0, 1000.0, 10_000.0]
        .into_iter()
        .find(|&scale| {
            overall.1 * scale <= 50_000.0
                && candidates.iter().flatten().flatten().all(|candidate| {
                    let score = candidate.score * scale;
                    (score - score.round()).abs() <= 1e-10
                })
        })
    else {
        return Ok(None);
    };
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
                    .map(|candidate| ((candidate.score * scale).round() as usize, candidate))
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
            search.consider()?;
            pieces.push(*candidate);
            score -= key;
        }
        output.push(pieces);
    }
    Ok(Some(output))
}

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

fn integer_bounds(bounds: Bounds, scale: f64) -> Option<(usize, usize)> {
    let low = ((bounds.0 - EPSILON) * scale).ceil().max(0.0);
    let high = ((bounds.1 + EPSILON) * scale).floor();
    (low <= high).then_some((low as usize, high as usize))
}
