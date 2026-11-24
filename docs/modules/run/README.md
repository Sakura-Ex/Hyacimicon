# Run settlement

`run-core` previews squad HP changes for completed ordinary timed nodes. Keep the battle outcome, termination reason, and remaining total action value (AV) from `BattleResult`, and call [`settle_ordinary_timed_node_hp`](../../../core/run/src/lib.rs) to calculate the HP changes. The caller that owns the run state decides when to apply them.

## Inputs and rules

Use this interface for ordinary timed nodes without an additional threshold penalty. Supply the effective base loss and progress loss coefficient, the squad's current HP and cap, the effective whole-number progress percentage after node adjustments, and the resolved recovery amount.

The 4.7 rules `90013` and `90017` specify a base loss of 5 and a progress loss coefficient of 10. The March7thHoney 4.7 reference calculation waives both losses at 100% progress. Below 100%, apply the base loss first, then round `(100 - effective progress) × coefficient / 100` down and apply the progress loss. A squad at zero HP skips recovery. Otherwise, recovery is limited by the HP cap. The preview reports the requested and applied amount for each step. [4.7 penalty table](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/ExcelOutput/GridFightPenaltyRule.json#L823-L956) · [4.7 reference settlement](https://github.com/Mar7thDev/March7thHoney/blob/d729daba9604e1e183e52519d0bc5590cb6f2bef/GameServer/Game/GridFight/GridFightSession.cs#L1835-L1883)

`Incomplete` means the battle has yet to reach settlement. `ProgressUnknown` means the effective progress is missing. `RecoveryUnknown` retains the two loss components and the HP after applying them. Pass `Some(0)` when the caller has established that no recovery applies. The caller resolves effective progress and talent activation. Verifying them against a complete in-game settlement remains separate from the reference calculation.

## Validation

Run `cargo test -p run-core --locked` from the repository root. The tests run the existing battle engine with synthetic fixtures and compare the HP preview with independently calculated values. A 150 AV fixture with rounds of 50 and 100 AV clears at AV 100 with 50 AV remaining, or times out at AV 150 with zero remaining after one enemy is defeated. With 20% effective progress, 2 points of recovery, and 92 starting HP, the losses are 5 and 8. The squad goes to 79 HP before recovery and finishes at 81 HP.
