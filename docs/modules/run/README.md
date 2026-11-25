# Run settlement

`run-core` previews squad HP changes for completed ordinary timed nodes. Keep the battle outcome, termination reason, and remaining total action value (AV) from `BattleResult`, and call [`settle_ordinary_timed_node_hp`](../../../core/run/src/lib.rs) to calculate the HP changes. The caller that owns the run state decides when to apply them.

## Inputs and rules

Use this interface for ordinary timed nodes without an additional threshold penalty. Supply the effective base loss and progress loss coefficient, the squad's current HP and cap, the effective whole-number progress percentage after node adjustments, and the resolved recovery amount.

The 4.7 rules `90013` and `90017` specify a base loss of 5 and a progress loss coefficient of 10. The March7thHoney 4.7 reference calculation waives both losses at 100% progress. Below 100%, apply the base loss first, then round `(100 - effective progress) × coefficient / 100` down and apply the progress loss. A squad at zero HP skips recovery. Otherwise, recovery is limited by the HP cap. The preview reports the requested and applied amount for each step. [4.7 penalty table](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/ExcelOutput/GridFightPenaltyRule.json#L823-L956) · [4.7 reference settlement](https://github.com/Mar7thDev/March7thHoney/blob/d729daba9604e1e183e52519d0bc5590cb6f2bef/GameServer/Game/GridFight/GridFightSession.cs#L1835-L1883)

`Incomplete` means the battle has yet to reach settlement. `ProgressUnknown` means the effective progress is missing. `RecoveryUnknown` retains the two loss components and the HP after applying them. Pass `Some(0)` when the caller has established that no recovery applies. The caller resolves effective progress and talent activation. Verifying them against a complete in-game settlement remains separate from the reference calculation.

## Preview HP after a scripted replay

Run the development example with a battle scenario, a seed, an [operation script](../battle-simulation/README.md), and a node context file. Use a scenario without a provisional `mechanics.settlement` policy.

```sh
cargo run -p run-core --example replay_timed_node --locked -- SCENARIO.json 42 OPERATIONS.json NODE.json
```

For a synthetic timeout with supplied 20% effective progress, `NODE.json` can contain the following values.

```json
{
  "hp_before": 92,
  "hp_cap": 100,
  "base_loss": 5,
  "progress_loss_coefficient": 10,
  "progress_percent": 20,
  "post_battle_healing": 2
}
```

Use `null` or omit the progress or recovery value to report it as unknown. Use `0` for known inactive recovery. The example outputs the original `battle` replay report, the supplied `node_context`, and `ordinary_timed_node_hp` from the same battle result. For a completed battle with the inputs above, the preview reports 5 base loss, 8 progress loss, 79 HP after losses, 2 recovery, and 81 final HP. The progress percentage is an input. The example runs a preview without applying the change to run state.

## Validation

Run `cargo test -p run-core --locked` from the repository root. The tests run the existing battle engine with synthetic fixtures and compare the HP preview with independently calculated values. A 150 AV fixture with rounds of 50 and 100 AV clears at AV 100 with 50 AV remaining, or times out at AV 150 with zero remaining after one enemy is defeated. With 20% effective progress, 2 points of recovery, and 92 starting HP, the losses are 5 and 8. The squad goes to 79 HP before recovery and finishes at 81 HP.

## 整局路线

`run_core::route` 保存 4.7 S2 的路线配置和阶段推进，区分补给、投资待选、正式节点进入和结束。该规则原型来自 `f6a56916`，保留其完整路线及错误阶段回归测试；正式运营接入应复用此路线或由同一版本规则数据生成路线，避免重复维护。早期独立 `match-session` crate 的向上取整结算不再接入。

普通计时节点 HP 预览仍按当前参考实现进行整数截断；早期玩家观察记录中的四舍五入、门槛、回血及保底顺序尚需核对，正式结算不得将两种算法视为已统一。详见[整局规则](../../game-data/match.md)及[观察记录](../../game-data/match-observations.md)。
