use crate::{BattleDiagnostic, BattleResolution};
use battle_core::{
    engine::simulate_cancellable,
    model::{BattleResult, Outcome, ValidatedScenario},
};

/// Summarize the declared basic_v1 budget without changing the engine outcome.
pub fn diagnose(
    input: &ValidatedScenario,
    result: &BattleResult,
) -> Result<BattleDiagnostic, String> {
    if input.fingerprint() != result.scenario_sha256 || input.scenario().ruleset != result.ruleset {
        return Err("battle diagnostic input identity mismatch".into());
    }
    let used = result.elapsed_av + result.penalty_av;
    let margin = input.scenario().max_av - used;
    if !used.is_finite()
        || !margin.is_finite()
        || result.elapsed_av < 0.0
        || result.penalty_av < 0.0
    {
        return Err("invalid battle diagnostic timing".into());
    }
    let remaining = margin.max(0.0);
    Ok(BattleDiagnostic {
        budget_limit_av: input.scenario().max_av,
        budget_used_av: used,
        budget_margin_av: margin,
        remaining_av: remaining,
        overrun_av: (-margin).max(0.0),
        success_remaining_av: (result.outcome == Outcome::Victory).then_some(remaining),
        failure_reason: (result.outcome == Outcome::Defeat).then_some(result.termination),
        incomplete_reason: (result.outcome == Outcome::Incomplete).then_some(result.termination),
    })
}

/// Execute exactly one engine call. No retries, batch averages, or inferred rules.
pub fn resolve_once(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResolution, String> {
    let result = simulate_cancellable(input, seed, trace, cancel)?;
    let diagnostic = diagnose(input, &result)?;
    Ok(BattleResolution { result, diagnostic })
}
