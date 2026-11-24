# 整局结算

`run-core` 提供已完成的限时普通节点的小队血量结算预览。调用方先完成单场战斗，保留 `BattleResult` 的节点胜负、结束原因与剩余总行动值，再调用 [`settle_ordinary_timed_node_hp`](../../../core/run/src/lib.rs) 得到小队血量变化。接口只计算一次预览，由整局状态的持有方决定何时入账。

## 输入与适用规则

该入口适用于没有额外门槛罚血的限时普通节点。调用方提供当前生效的基础扣血、进度扣血系数、小队血量及其上限。进度使用场景规则处理后的有效整数百分比。回血量使用已经确认生效的数值。

规则依据是 4.7 普通节点的公开罚则 `90013`、`90017` 与 March7thHoney 4.7 的参考计算。两个罚则的基础扣血均为 5、进度扣血系数均为 10。进度达到 100% 时免两项扣血。进度未满时先扣基础值，再取 `(100 - 有效进度) × 进度系数 / 100` 向下取整。小队血量扣至零后跳过回血，存活时按血量上限裁剪回血。结算保留每项应扣或应回和实际变化。 [4.7 公开罚则](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/ExcelOutput/GridFightPenaltyRule.json#L823-L956) · [4.7 参考结算](https://github.com/Mar7thDev/March7thHoney/blob/d729daba9604e1e183e52519d0bc5590cb6f2bef/GameServer/Game/GridFight/GridFightSession.cs#L1835-L1883)

`Incomplete` 表示战斗未完成，当前调用还未到结算阶段。`ProgressUnknown` 表示有效进度缺失。`RecoveryUnknown` 保留已确定的两项扣血和扣血后血量。明确无回血时传入 `Some(0)`，将得到完整结算。输入的有效进度和天赋状态由调用方解析。真实样本的进度形成过程与完整结算仍待同场核对。

## 验证

在仓库根目录运行 `cargo test -p run-core --locked`。测试借用战斗模块的开发样例取得真实引擎输出，使用手算预期对照结算。在 150 AV、轮次为 50 和 100 的人造场景，AV 100 清场时剩 50 AV，只打掉一名小怪时在 AV 150 超时、余额为零。提供有效进度 20%、战后回血 2 点和起始 92 HP 后，扣血明细为 5 和 8，扣后 79，最终 81。
