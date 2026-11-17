//! Single-owner event loop for the explicit battle_v2 contract.
use crate::{
    decision::*,
    elements::{DamageElement, Element},
    engine::timeline::{self as scheduler, Timeline, DISTANCE},
    math,
    model::*,
    modifiers::{ModifierBook, Property},
};
use rand_chacha::ChaCha8Rng;

mod damage;
mod effects;
mod events;
mod operations;
mod result;
mod rng;
mod state;
mod targets;
mod timeline;
mod values;

use effects::finite;
use rng::{stream, uniform};
use state::{CombatUnit, Pending, State};

pub const RNG_ALGORITHM: &str =
    "ChaCha8Rng/rand_chacha=0.9.0; SHA256-separated target/critical/element streams v1";
const TRACE_LIMIT: usize = 100_000;

/// Run a single scenario without cancellation; the input remains unchanged.
pub fn simulate(input: &ValidatedScenario, seed: u64, trace: bool) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, trace, &mut || false)
}
/// Check cancellation before each action, including immediate ultimate chains.
/// Checks occur at action boundaries before selecting an action or advancing AV,
/// never partway through an action's effects. A callback returning `true` yields
/// `Ok(BattleResult)` with [`Outcome::Incomplete`] and [`Termination::Cancelled`].
/// Settled terminal elimination and already exhausted AV take precedence and
/// return their results without polling cancellation at that boundary.
pub fn simulate_cancellable(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    simulate_with_policy(input, seed, trace, cancel, &mut Automatic)
}

/// Execute through legal operations; policy cannot mutate battle state.
pub fn simulate_with_policy(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl Policy,
) -> Result<BattleResult, String> {
    let mut state = State::new(input, seed, trace);
    state.dispatch(BattleEvent::BattleStart, None, None)?;
    state.dispatch(BattleEvent::WaveStart, None, None)?;
    loop {
        let allies = state.units.iter().any(|u| u.ally && u.alive());
        // A queued instance counts as an enemy even during an empty-board boundary.
        let queue_waiting = input.scenario().mechanics.queue_capacity.is_some()
            && state.queue_next < input.scenario().waves[0].len();
        let enemies = queue_waiting || state.units.iter().any(|u| !u.ally && u.alive());
        let cleared = state.wave + usize::from(!enemies);
        if !allies && !enemies {
            return Ok(state.finish(seed, Outcome::Draw, Termination::BothDefeated, cleared));
        }
        if !allies {
            return Ok(state.finish(seed, Outcome::Defeat, Termination::AlliesDefeated, cleared));
        }
        if !enemies {
            if state.wave + 1 == input.scenario().waves.len() {
                return Ok(state.finish(
                    seed,
                    Outcome::Victory,
                    Termination::EnemiesDefeated,
                    cleared,
                ));
            }
            state.wave += 1;
            for u in &input.scenario().waves[state.wave] {
                let unit = CombatUnit::new(u, false, &input.scenario().mechanics);
                state.units.push(unit);
            }
            state.dispatch(BattleEvent::WaveStart, None, None)?;
            continue;
        }
        // Deadline is settled before safety/cancellation, after complete actions.
        if state.exhausted {
            return Ok(state.finish(
                seed,
                Outcome::Defeat,
                Termination::AvBudgetExhausted,
                cleared,
            ));
        }
        let zero_time_due = input.scenario().mechanics.unit_before_cycle
            && (state.turn.is_some()
                || !state.pending.is_empty()
                || state.units.iter().any(|u| u.alive() && u.remaining == 0.0));
        if state.timeline.remaining == 0.0 && !zero_time_due {
            state.emit(
                "cycle_end",
                0,
                None,
                None,
                Some((state.timeline.cycle + 1) as f64),
                None,
            );
            state.dispatch(BattleEvent::CycleEnd, None, None)?;
            if !state.timeline.next_cycle() {
                state.exhausted = true;
            }
            continue;
        }
        if cancel() {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::Cancelled, cleared));
        }
        if state.actions >= input.scenario().max_actions {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::ActionLimit, cleared));
        }
        state.refill_queue();
        if let Some(op) = state.decide(DecisionPoint::Insert, None, policy)? {
            state.enqueue(Pending {
                actor: op.actor,
                kind: RequestKind::Ultimate,
                action: Some(op.action),
            })?;
        }
        if let Some(request) = state.pending.pop_front() {
            let actor = request.actor;
            if !state.units[actor].alive() {
                state.emit("request_invalid", actor, None, None, None, None);
                continue;
            }
            let selected = if request.kind == RequestKind::ExtraTurn {
                state.emit("extra_turn_start", actor, None, None, None, None);
                state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                if !state.units[actor].alive() {
                    continue;
                }
                state
                    .decide(DecisionPoint::ExtraTurn, Some(actor), policy)?
                    .map(|op| op.action)
            } else {
                request.action
            };
            let action = selected.ok_or("turn policy declined all legal actions")?;
            if !state.affordable(actor, &state.units[actor].spec.actions[action]) {
                state.emit("request_invalid", actor, None, None, None, None);
                continue;
            }
            if request.kind == RequestKind::Ultimate {
                let skill = state.units[actor].spec.actions[action].clone();
                state.pay(actor, &skill);
                state.emit(
                    "ultimate_turn_start",
                    actor,
                    None,
                    None,
                    Some(state.units[actor].energy),
                    None,
                );
                state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
                if !state.units[actor].alive() {
                    continue;
                }
            }
            state.act(actor, action, request.kind == RequestKind::Ultimate)?;
            if request.kind != RequestKind::FollowUp {
                state.dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
                state.emit("extra_turn_end", actor, None, None, None, None);
            }
            continue;
        }
        if let Some(actor) = state.turn {
            state.dispatch(BattleEvent::TurnEnd, Some(actor), None)?;
            state.units[actor].remaining = (DISTANCE - state.units[actor].next_shift).max(0.0);
            state.units[actor].next_shift = 0.0;
            state.turn = None;
            state.emit(
                "normal_turn_end",
                actor,
                None,
                None,
                Some(state.units[actor].remaining),
                None,
            );
            continue;
        }
        let (actor, wait) = scheduler::next_normal(
            state
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| u.alive())
                .map(|(i, u)| (i, u.remaining, u.spec.stats.speed)),
        )
        .ok_or("empty axis")?;
        let cycle_first = state.timeline.remaining < wait
            || (state.timeline.remaining == wait && !input.scenario().mechanics.unit_before_cycle);
        let delta = wait.min(state.timeline.remaining);
        state.timeline.advance(delta);
        for u in &mut state.units {
            if u.alive() {
                u.remaining = (u.remaining - delta * u.spec.stats.speed).max(0.0);
            }
        }
        if cycle_first {
            continue;
        }
        state.turn = Some(actor);
        state.emit(
            "normal_turn_start",
            actor,
            None,
            None,
            Some(state.units[actor].remaining),
            None,
        );
        if state.units[actor].shield_turns > 0 {
            state.units[actor].shield_turns -= 1;
            if state.units[actor].shield_turns == 0 {
                let expired = state.units[actor].shield;
                state.units[actor].shield = 0.0;
                state.emit(
                    "shield_expired",
                    actor,
                    Some(actor),
                    None,
                    Some(expired),
                    None,
                );
            }
        }
        state.dispatch(BattleEvent::TurnStart, Some(actor), None)?;
        if !state.units[actor].alive() {
            continue;
        }
        let op = state
            .decide(DecisionPoint::NormalTurn, Some(actor), policy)?
            .ok_or("turn policy declined all legal actions")?;
        state.act(actor, op.action, false)?;
    }
}
