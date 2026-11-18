use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AccountReference, AccountRequest, CharacterSpec, GeneratedAccount, GenerationError,
    GoldCategory, Search, assemble, invalid,
    reference::{Cone, Profile, Team},
};

struct Candidate<'a> {
    profile: &'a Profile,
    spec: CharacterSpec,
    builds: Vec<Build<'a>>,
    costs: Vec<usize>,
    cone_count: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Build<'a> {
    pub cone: &'a Cone,
    pub eidolons: u8,
    pub stack: u8,
    units: usize,
    cone_units: usize,
}

pub(super) struct Choice<'a> {
    pub profile: &'a Profile,
    pub spec: &'a CharacterSpec,
    pub build: Build<'a>,
}

pub(super) fn select(
    request: &AccountRequest,
    reference: &AccountReference,
    search: &mut Search<'_>,
) -> Result<GeneratedAccount, GenerationError> {
    let menu = menus(request, reference)?;
    let mut selected = unique_indices(&request.core_character_ids, &menu)?;
    let mut cores = selected.clone();
    for index in unique_indices(request.character_overrides.keys(), &menu)? {
        if !selected.contains(&index) {
            selected.push(index);
        }
    }
    if selected.len() > request.character_count {
        return Err(invalid("角色数量不足以容纳核心与覆盖角色"));
    }
    if menu.len() < request.character_count {
        return Err(GenerationError::NoSolution);
    }
    let available: Vec<_> = (0..menu.len())
        .filter(|index| !selected.contains(index))
        .collect();
    let target = request.gold_count.map(|gold| (gold * 2.0) as usize);
    if let Some(target) = target
        && !roster_fits(
            &menu,
            &selected,
            &available,
            request.character_count - selected.len(),
            target,
        )
    {
        return Err(GenerationError::NoSolution);
    }
    let automatic_cores = cores.is_empty() && request.character_count > 0;
    let all_cores = automatic_cores && request.core_count == request.character_count;
    let core_count = if automatic_cores && !all_cores {
        request.core_count
    } else {
        cores.len()
    };
    let mut plans = selected
        .iter()
        .filter_map(|&index| team(menu[index].profile, search))
        .collect();
    let mut roster = RosterSearch {
        request,
        reference,
        menu: &menu,
        target,
        core_count,
        all_cores,
        fixed_roster: request.character_count == menu.len(),
        selected: &mut selected,
        cores: &mut cores,
        plans: &mut plans,
        search,
    };
    roster
        .choose(available, (0..menu.len()).collect())?
        .ok_or(GenerationError::NoSolution)
}

fn unique_indices<'a>(
    ids: impl IntoIterator<Item = &'a String>,
    menus: &[Candidate<'_>],
) -> Result<Vec<usize>, GenerationError> {
    let mut result = Vec::new();
    for id in ids {
        let index = menus
            .iter()
            .position(|c| c.profile.id == *id)
            .ok_or_else(|| invalid(format!("核心或覆盖角色 {id} 不在可用候选范围内")))?;
        if !result.contains(&index) {
            result.push(index);
        }
    }
    Ok(result)
}

fn menus<'a>(
    request: &AccountRequest,
    reference: &'a AccountReference,
) -> Result<Vec<Candidate<'a>>, GenerationError> {
    let characters = pool(
        request.character_pool.as_deref(),
        &reference.characters,
        "角色",
    )?;
    let cones = pool(
        request.light_cone_pool.as_deref(),
        &reference.light_cones,
        "光锥",
    )?;
    let mut result = Vec::new();
    for profile in characters {
        let (spec, fixed) = if let Some(spec) = request.character_overrides.get(&profile.id) {
            (spec.clone(), true)
        } else if let Some((eidolons, stack)) =
            request.five_star_build.filter(|_| profile.rarity == 5)
        {
            let mut spec = CharacterSpec {
                eidolons,
                ..CharacterSpec::default()
            };
            spec.light_cone.light_cone_id = profile.signature.clone();
            spec.light_cone.superimposition = stack;
            (spec, true)
        } else {
            (CharacterSpec::default(), false)
        };
        let cone_choices = if let Some(id) = &spec.light_cone.light_cone_id {
            vec![
                reference
                    .light_cones
                    .get(id)
                    .ok_or_else(|| invalid(format!("未知光锥 {id}")))?,
            ]
        } else {
            cones
                .iter()
                .copied()
                .filter(|cone| cone.path == profile.path)
                .collect()
        };
        if cone_choices.is_empty() {
            continue;
        }
        let eidolons = if fixed {
            spec.eidolons..=spec.eidolons
        } else {
            0..=6
        };
        let stack = if fixed {
            spec.light_cone.superimposition..=spec.light_cone.superimposition
        } else {
            1..=5
        };
        let mut builds = Vec::new();
        let mut costs = BTreeSet::new();
        for &cone in &cone_choices {
            for eidolons in eidolons.clone() {
                let body_units = gold_units(profile.category, eidolons + 1, request);
                for stack in stack.clone() {
                    let cone_units = gold_units(cone.category, stack, request);
                    let units = body_units + cone_units;
                    builds.push(Build {
                        cone,
                        eidolons,
                        stack,
                        units,
                        cone_units,
                    });
                    costs.insert(units);
                }
            }
        }
        result.push(Candidate {
            profile,
            spec,
            builds,
            costs: costs.into_iter().collect(),
            cone_count: cone_choices.len(),
        });
    }
    Ok(result)
}

fn pool<'a, T>(
    ids: Option<&[String]>,
    source: &'a BTreeMap<String, T>,
    label: &str,
) -> Result<Vec<&'a T>, GenerationError> {
    let Some(ids) = ids else {
        return Ok(source.values().collect());
    };
    let mut seen = BTreeSet::new();
    ids.iter()
        .filter(|id| seen.insert(*id))
        .map(|id| {
            source
                .get(id)
                .ok_or_else(|| invalid(format!("未知{label} {id}")))
        })
        .collect()
}

fn gold_units(category: GoldCategory, quantity: u8, request: &AccountRequest) -> usize {
    if request.gold_categories.contains(&category) {
        category.half_units() * usize::from(quantity)
    } else {
        0
    }
}

fn roster_fits(
    menu: &[Candidate<'_>],
    selected: &[usize],
    available: &[usize],
    needed: usize,
    target: usize,
) -> bool {
    if available.len() < needed
        || target
            > selected
                .iter()
                .chain(available)
                .map(|&index| menu[index].costs.last().copied().unwrap_or_default())
                .sum::<usize>()
    {
        return false;
    }
    let mut fixed = vec![false; target + 1];
    fixed[0] = true;
    let (fixed_indices, optional) = if needed == available.len() {
        (selected.iter().chain(available), &[][..])
    } else {
        (selected.iter().chain(&[]), available)
    };
    for &index in fixed_indices {
        let mut next = vec![false; target + 1];
        extend_costs(&fixed, &menu[index].costs, &mut next);
        fixed = next;
    }
    if optional.is_empty() {
        return fixed[target];
    }
    let mut counts = vec![vec![false; target + 1]; needed + 1];
    counts[0] = fixed;
    for (index, &candidate) in optional.iter().enumerate() {
        for size in (1..=needed.min(index + 1)).rev() {
            let (earlier, later) = counts.split_at_mut(size);
            extend_costs(&earlier[size - 1], &menu[candidate].costs, &mut later[0]);
        }
    }
    counts[needed][target]
}

fn extend_costs(previous: &[bool], costs: &[usize], next: &mut [bool]) {
    let limit = next.len();
    for &cost in costs.iter().take_while(|&&cost| cost < limit) {
        for (destination, &reachable) in next[cost..].iter_mut().zip(previous) {
            *destination |= reachable;
        }
    }
}

struct RosterSearch<'a, 'b, 'rng> {
    request: &'a AccountRequest,
    reference: &'a AccountReference,
    menu: &'a [Candidate<'a>],
    target: Option<usize>,
    core_count: usize,
    all_cores: bool,
    fixed_roster: bool,
    selected: &'b mut Vec<usize>,
    cores: &'b mut Vec<usize>,
    plans: &'b mut Vec<&'a Team>,
    search: &'b mut Search<'rng>,
}

impl RosterSearch<'_, '_, '_> {
    fn choose(
        &mut self,
        available: Vec<usize>,
        mut core_candidates: Vec<usize>,
    ) -> Result<Option<GeneratedAccount>, GenerationError> {
        let needed = self.request.character_count - self.selected.len();
        let cores_needed = self.core_count - self.cores.len();
        if needed == 0 && cores_needed == 0 {
            return self.generate();
        }
        let picking_core = cores_needed > 0;
        let mut candidates = if picking_core {
            core_candidates.retain(|index| {
                self.selected.contains(index) || (needed > 0 && available.contains(index))
            });
            core_candidates
        } else {
            available.clone()
        };
        let threshold = if picking_core { cores_needed } else { needed };
        let chosen_ids: BTreeSet<_> = self
            .selected
            .iter()
            .map(|&i| self.menu[i].profile.id.as_str())
            .collect();
        let teammates = teammate_scores(self.plans, &chosen_ids);
        while candidates.len() >= threshold {
            let option = self.draw(&candidates, &teammates);
            let index = candidates.remove(option);
            self.search.consider()?;
            let added = !self.selected.contains(&index);
            let rest = if picking_core {
                available
                    .iter()
                    .copied()
                    .filter(|&other| other != index)
                    .collect()
            } else {
                candidates.clone()
            };
            if added {
                self.selected.push(index);
                if !self.fixed_roster
                    && self.target.is_some_and(|target| {
                        !roster_fits(self.menu, self.selected, &rest, needed - 1, target)
                    })
                {
                    self.selected.pop();
                    continue;
                }
            }
            if picking_core {
                self.cores.push(index);
            }
            let plan = if added {
                team(self.menu[index].profile, self.search)
            } else {
                None
            };
            self.plans.extend(plan);
            let result = self.choose(
                rest,
                if picking_core {
                    candidates.clone()
                } else {
                    Vec::new()
                },
            )?;
            if result.is_some() {
                return Ok(result);
            }
            if plan.is_some() {
                self.plans.pop();
            }
            if picking_core {
                self.cores.pop();
            }
            if added {
                self.selected.pop();
            }
        }
        Ok(None)
    }

    fn draw(&mut self, candidates: &[usize], scores: &BTreeMap<&str, f64>) -> usize {
        let weight = |index: usize| {
            scores
                .get(self.menu[index].profile.id.as_str())
                .copied()
                .unwrap_or(0.0)
        };
        let sum: f64 = candidates.iter().map(|&index| weight(index)).sum();
        if sum == 0.0 || self.search.unit() >= self.request.teammate_bias {
            return self.search.index(candidates.len());
        }
        let mut pick = self.search.unit() * sum;
        let mut last = 0;
        for (position, &index) in candidates.iter().enumerate() {
            let value = weight(index);
            if value > 0.0 {
                last = position;
                if pick < value {
                    return position;
                }
                pick -= value;
            }
        }
        last
    }

    fn generate(&mut self) -> Result<Option<GeneratedAccount>, GenerationError> {
        let mut order = self.selected.clone();
        order.sort_by_key(|&index| self.menu[index].cone_count);
        let core_indices = if self.all_cores {
            &*self.selected
        } else {
            &*self.cores
        };
        let core_ids: Vec<_> = core_indices
            .iter()
            .map(|&i| self.menu[i].profile.id.clone())
            .collect();
        let choices = choose_builds(
            self.menu,
            &order,
            &core_ids,
            self.request,
            self.target,
            self.search,
        )?;
        let Some(choices) = choices else {
            return Ok(None);
        };
        match assemble(
            self.request,
            self.reference,
            &choices,
            core_ids,
            self.search,
        ) {
            Ok(result) => Ok(Some(result)),
            Err(GenerationError::NoSolution) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn team<'a>(profile: &'a Profile, search: &mut Search<'_>) -> Option<&'a Team> {
    (!profile.teams.is_empty()).then(|| &profile.teams[search.index(profile.teams.len())])
}

fn teammate_scores<'a>(plans: &[&'a Team], selected: &BTreeSet<&str>) -> BTreeMap<&'a str, f64> {
    let mut result = BTreeMap::new();
    for plan in plans {
        let slots: Vec<Vec<_>> = plan
            .members
            .iter()
            .zip(&plan.backups)
            .map(|(lead, backups)| {
                std::iter::once(lead)
                    .chain(backups)
                    .map(String::as_str)
                    .collect()
            })
            .collect();
        let mut assigned = BTreeMap::new();
        for (slot, options) in slots.iter().enumerate() {
            if !match_slot(slot, &slots, selected, &mut assigned, &mut BTreeSet::new()) {
                for (position, &id) in options.iter().enumerate() {
                    *result.entry(id).or_default() += if position == 0 { 1.0 } else { 0.5 };
                }
            }
        }
    }
    result
}

fn match_slot<'a>(
    slot: usize,
    slots: &[Vec<&'a str>],
    selected: &BTreeSet<&str>,
    assigned: &mut BTreeMap<&'a str, usize>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    for &id in &slots[slot] {
        if selected.contains(id)
            && visited.insert(id)
            && assigned
                .get(id)
                .copied()
                .is_none_or(|previous| match_slot(previous, slots, selected, assigned, visited))
        {
            assigned.insert(id, slot);
            return true;
        }
    }
    false
}

fn choose_builds<'a>(
    menu: &'a [Candidate<'a>],
    order: &[usize],
    cores: &[String],
    request: &AccountRequest,
    target: Option<usize>,
    search: &mut Search<'_>,
) -> Result<Option<Vec<Choice<'a>>>, GenerationError> {
    // 指定总金数已由 roster_fits 检查。这里的后缀表用于剪枝光锥分配。
    let cap = target.unwrap_or_else(|| {
        order
            .iter()
            .map(|&i| menu[i].costs.last().copied().unwrap_or_default())
            .sum()
    });
    let mut suffix = vec![vec![false; cap + 1]; order.len() + 1];
    suffix[order.len()][0] = true;
    for (position, &index) in order.iter().enumerate().rev() {
        let (left, right) = suffix.split_at_mut(position + 1);
        extend_costs(&right[0], &menu[index].costs, &mut left[position]);
    }
    let amounts = if let Some(target) = target {
        vec![target]
    } else {
        let mut amounts: Vec<_> = (0..=cap).filter(|&amount| suffix[0][amount]).collect();
        search.shuffle(&mut amounts);
        amounts
    };
    let bounds: Vec<_> = order
        .iter()
        .map(|&i| {
            let costs = &menu[i].costs;
            (costs[0] as f64, costs[costs.len() - 1] as f64)
        })
        .collect();
    let strength = request
        .gold_concentration
        .unwrap_or(request.resource_concentration);
    let weights = weights(
        order.iter().map(|&i| menu[i].profile.id.as_str()),
        cores,
        strength,
    );
    for amount in amounts {
        let desired = distribute(amount as f64, &bounds, &weights);
        let mut builds = BuildSearch {
            menu,
            order,
            suffix: &suffix,
            desired: &desired,
            light_cone_share: request.light_cone_gold_share,
            selected: Vec::new(),
            used_unique: BTreeSet::new(),
            search,
        };
        if builds.solve(amount)? {
            return Ok(Some(builds.selected));
        }
    }
    Ok(None)
}

struct BuildSearch<'a, 'b, 'rng> {
    menu: &'a [Candidate<'a>],
    order: &'b [usize],
    suffix: &'b [Vec<bool>],
    desired: &'b [f64],
    light_cone_share: f64,
    selected: Vec<Choice<'a>>,
    used_unique: BTreeSet<&'a str>,
    search: &'b mut Search<'rng>,
}

impl BuildSearch<'_, '_, '_> {
    fn solve(&mut self, remaining: usize) -> Result<bool, GenerationError> {
        let position = self.selected.len();
        let Some(&index) = self.order.get(position) else {
            return Ok(remaining == 0);
        };
        let candidate = &self.menu[index];
        let mut options: Vec<_> = candidate
            .builds
            .iter()
            .copied()
            .filter(|build| {
                remaining
                    .checked_sub(build.units)
                    .is_some_and(|rest| self.suffix[position + 1][rest])
                    && !self.used_unique.contains(build.cone.id.as_str())
            })
            .map(|build| {
                let rank = (
                    (build.units as f64 - self.desired[position]).abs(),
                    (build.cone_units as f64 - build.units as f64 * self.light_cone_share).abs(),
                    !candidate.profile.recommended.contains(&build.cone.id),
                    self.search.unit(),
                );
                (build, rank)
            })
            .collect();
        options.sort_by(|(_, a), (_, b)| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.total_cmp(&b.3))
        });
        let mut examined = BTreeSet::new();
        for (build, _) in options {
            let unique = build.cone.unique.then_some(build.cone.id.as_str());
            if !examined.insert((build.units, unique)) {
                continue;
            }
            self.search.consider()?;
            self.selected.push(Choice {
                profile: candidate.profile,
                spec: &candidate.spec,
                build,
            });
            self.used_unique.extend(unique);
            if self.solve(remaining - build.units)? {
                return Ok(true);
            }
            if let Some(id) = unique {
                self.used_unique.remove(id);
            }
            self.selected.pop();
        }
        Ok(false)
    }
}

pub(super) fn weights<'a>(
    ids: impl ExactSizeIterator<Item = &'a str>,
    cores: &[String],
    strength: f64,
) -> Vec<f64> {
    let base = (1.0 - strength) / ids.len().max(1) as f64;
    let extra = strength / cores.len().max(1) as f64;
    ids.map(|id| {
        base + if cores.iter().any(|core| core == id) {
            extra
        } else {
            0.0
        }
    })
    .collect()
}

pub(super) fn distribute(total: f64, bounds: &[(f64, f64)], weights: &[f64]) -> Vec<f64> {
    let mut values: Vec<_> = bounds.iter().map(|&(min, _)| min).collect();
    let mut remaining = total - values.iter().sum::<f64>();
    let mut active: Vec<_> = (0..bounds.len())
        .filter(|&index| bounds[index].1 > values[index])
        .collect();
    while remaining > 1e-10 && !active.is_empty() {
        let weight_sum: f64 = active.iter().map(|&index| weights[index]).sum();
        let equal_weight = 1.0 / active.len() as f64;
        let mut visited_limit = false;
        let available = remaining;
        active.retain(|&index| {
            let fraction = if weight_sum == 0.0 {
                equal_weight
            } else {
                weights[index] / weight_sum
            };
            let capacity = bounds[index].1 - values[index];
            if available * fraction >= capacity {
                values[index] = bounds[index].1;
                remaining -= capacity;
                visited_limit = true;
                false
            } else {
                true
            }
        });
        if !visited_limit {
            for index in active {
                let fraction = if weight_sum == 0.0 {
                    equal_weight
                } else {
                    weights[index] / weight_sum
                };
                values[index] += remaining * fraction;
            }
            break;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChaCha8Rng, LightConeSpec, SeedableRng, generate_account};

    #[test]
    fn preferred_teammates_only_fill_one_recommended_slot_per_character() {
        let mut reference = AccountReference::bundled().unwrap();
        reference.characters.get_mut("1004").unwrap().teams = vec![Team {
            members: vec!["1005".into(), "1103".into()],
            backups: [vec!["8009".into()], vec!["8009".into()], Vec::new()],
        }];
        reference.characters.get_mut("8009").unwrap().teams.clear();
        let request = AccountRequest {
            character_pool: Some(vec![
                "1004".into(),
                "1005".into(),
                "1103".into(),
                "8009".into(),
            ]),
            core_character_ids: vec!["1004".into(), "8009".into()],
            teammate_bias: 1.0,
            ..AccountRequest::new("teammates", 3)
        };
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        let roster: BTreeSet<_> = result
            .account
            .characters
            .iter()
            .map(|character| character.character_id.as_str())
            .collect();
        assert_eq!(roster, BTreeSet::from(["1004", "8009", "1103"]));
    }

    #[test]
    fn roster_retries_when_the_first_choice_cannot_complete_equipment_or_score() {
        let mut reference = AccountReference::bundled().unwrap();
        reference.characters.get_mut("1004").unwrap().teams = vec![Team {
            members: vec!["1005".into()],
            backups: std::array::from_fn(|_| Vec::new()),
        }];
        let request = AccountRequest {
            character_pool: Some(vec!["1004".into(), "1005".into(), "1103".into()]),
            core_character_ids: vec!["1004".into()],
            teammate_bias: 1.0,
            light_cone_pool: Some(vec!["24003".into(), "21006".into()]),
            character_overrides: BTreeMap::from([(
                "1004".into(),
                CharacterSpec {
                    light_cone: LightConeSpec {
                        light_cone_id: Some("24003".into()),
                        ..LightConeSpec::default()
                    },
                    ..CharacterSpec::default()
                },
            )]),
            ..AccountRequest::new("unique-retry", 2)
        };
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        let roster: BTreeSet<_> = result
            .account
            .characters
            .iter()
            .map(|character| character.character_id.as_str())
            .collect();
        assert_eq!(roster, BTreeSet::from(["1004", "1103"]));

        reference
            .characters
            .get_mut("1004")
            .unwrap()
            .relics
            .weights
            .clear();
        let request = AccountRequest {
            character_pool: Some(vec!["1004".into(), "1005".into()]),
            core_count: 0,
            min_mean_effective_rolls: Some(1.0),
            ..AccountRequest::new("score-retry", 1)
        };
        let seed = (0..16)
            .find(|&seed| {
                let mut rng = ChaCha8Rng::seed_from_u64(seed);
                let mut search = Search {
                    rng: &mut rng,
                    attempts: 0,
                    limit: 1,
                };
                search.index(2) == 0
            })
            .unwrap();
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
        assert_eq!(result.account.characters[0].character_id, "1005");
        assert!(result.mean_effective_rolls() >= 1.0);
        assert_eq!(result.attempts, 10);
    }
}
