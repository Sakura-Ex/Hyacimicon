//! Match route progression and squad-health settlement reference.

pub mod route;

use battle_core::model::{BattleResult, Outcome};

/// Effective rule parameters for an ordinary timed node with no threshold penalty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrdinaryTimedNodeHpRule {
    /// Squad HP requested as a base loss when effective progress is below 100%.
    pub base_loss: u32,
    /// Squad HP requested for the unfinished fraction, before integer truncation.
    pub progress_loss_coefficient: u32,
}

/// Squad state and inputs resolved by the caller for the completed node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimedNodeHpInput {
    /// Squad HP immediately before the node settles.
    pub hp_before: u32,
    /// Effective squad HP cap at settlement.
    pub hp_cap: u32,
    /// Effective rule for the selected ordinary timed node.
    pub rule: OrdinaryTimedNodeHpRule,
    /// Effective integer progress percent after node-specific adjustments, or unknown.
    pub progress_percent: Option<u8>,
    /// Resolved recovery amount. Use Some(0) for known inactive and None for unknown.
    pub post_battle_healing: Option<u32>,
}

/// A requested HP change and its magnitude after the applicable HP limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HpChange {
    /// Magnitude before limiting it to the available HP or healing capacity.
    pub requested: u32,
    /// Magnitude actually applied.
    pub applied: u32,
}

/// Known losses applied in order, followed by the remaining squad HP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SquadHpLosses {
    /// Base loss.
    pub base: HpChange,
    /// Progress loss, applied after the base loss.
    pub progress: HpChange,
    /// Squad HP after both loss components and before any recovery.
    pub hp_after_losses: u32,
}

/// Preview of the squad HP changes, with incomplete or unresolved inputs made explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedNodeHpSettlement {
    /// The simulation stopped before a battle outcome was reached.
    Incomplete,
    /// The effective progress is unknown. No loss is calculated.
    ProgressUnknown,
    /// Losses can be calculated while the recovery amount is unknown.
    RecoveryUnknown {
        /// Known losses and the HP before recovery.
        losses: SquadHpLosses,
    },
    /// A complete squad HP preview.
    Settled {
        /// Losses and HP before recovery.
        losses: SquadHpLosses,
        /// Recovery after applying the HP cap. None means HP reached zero before this step.
        recovery: Option<HpChange>,
        /// Squad HP after settlement.
        hp_after: u32,
    },
}

/// Calculate a squad HP preview using the 4.7 ordinary timed-node reference rule.
///
/// The caller supplies the effective rule, progress and recovery. The battle
/// outcome only determines whether settlement has been reached. This function
/// performs no write to the run or to the battle result.
pub fn settle_ordinary_timed_node_hp(
    battle: &BattleResult,
    input: TimedNodeHpInput,
) -> Result<TimedNodeHpSettlement, &'static str> {
    if battle.outcome == Outcome::Incomplete {
        return Ok(TimedNodeHpSettlement::Incomplete);
    }
    if input.hp_cap == 0 || input.hp_before > input.hp_cap {
        return Err("squad HP must fit within a positive HP cap");
    }
    let Some(progress) = input.progress_percent else {
        return Ok(TimedNodeHpSettlement::ProgressUnknown);
    };
    let unfinished = 100u8
        .checked_sub(progress)
        .ok_or("effective progress must be between 0 and 100 percent")?;
    let base_requested = if unfinished == 0 {
        0
    } else {
        input.rule.base_loss
    };
    // The widened product fits in u64. Dividing by 100 brings the result
    // back within the coefficient's u32 range because unfinished is at most 100.
    let progress_requested =
        (u64::from(unfinished) * u64::from(input.rule.progress_loss_coefficient) / 100) as u32;
    let base_applied = base_requested.min(input.hp_before);
    let hp_after_base = input.hp_before - base_applied;
    let progress_applied = progress_requested.min(hp_after_base);
    let hp_after_losses = hp_after_base - progress_applied;
    let losses = SquadHpLosses {
        base: HpChange {
            requested: base_requested,
            applied: base_applied,
        },
        progress: HpChange {
            requested: progress_requested,
            applied: progress_applied,
        },
        hp_after_losses,
    };

    if hp_after_losses == 0 {
        return Ok(TimedNodeHpSettlement::Settled {
            losses,
            recovery: None,
            hp_after: 0,
        });
    }
    let Some(recovery_requested) = input.post_battle_healing else {
        return Ok(TimedNodeHpSettlement::RecoveryUnknown { losses });
    };
    let recovery_applied = recovery_requested.min(input.hp_cap - hp_after_losses);
    Ok(TimedNodeHpSettlement::Settled {
        losses,
        recovery: Some(HpChange {
            requested: recovery_requested,
            applied: recovery_applied,
        }),
        hp_after: hp_after_losses + recovery_applied,
    })
}
