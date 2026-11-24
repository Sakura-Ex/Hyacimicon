# 战斗核心与 CLI 使用手册

`battle-core` 是崩铁货币战争 AI 系统的独立战斗模拟核心，提供纯 Rust 编写的确定性战斗规则演算、蒙特卡洛批次评估以及命令行工具 `hcwa-battle`。

默认构建接收已组装的 Rust `Scenario`，执行单场战斗或批次评估。启用 `diagnostics` 特性后，可读取开发用 JSON、生成输入指纹并导出回放。

系统设计、分步 API 和验证入口见[战斗模拟模块文档](README.md)。

---

## 1. 命令行工具 (`hcwa-battle`)

命令行工具 [`hcwa-battle`](../../../core/battle/src/main.rs) 要求启用 `diagnostics` 特性，用于解析开发场景、重放单局和批次评估。

### 1.1 子命令手册

从仓库根目录执行（统一使用锁定工具链）：

```powershell
# 1. 解析开发场景并输出场景指纹、来源与随机算法
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- inspect <SCENARIO.json>

# 2. 单局确定性模拟（参数：场景文件 随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- simulate <SCENARIO.json> <SEED_U64>

# 3. 蒙特卡洛批次评估（参数：场景文件 试验场次 主随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- batch <SCENARIO.json> <TRIALS> <MASTER_SEED_U64>

# 4. 按明确的操作序列逐步回放
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- replay core/battle/examples/team-waves.json 42 core/battle/examples/team-waves-operations.json
```

### 1.2 退出码与错误输出

- **退出码 0**：执行成功，标准输出（stdout）打印格式化的 JSON 结果。
- **退出码 2**：文件读取、结构解析、参数、模拟执行或输出出错，错误详情以 JSON 写入标准错误（stderr）。
- 建议将大量批次运行结果重定向至本地忽略目录，如 `runs/result.json`。

### 1.3 固定操作回放

`replay SCENARIO.json SEED OPERATIONS.json` 使用相同的 `BattleSession` 执行外部指定的操作。操作文件使用 `schema_version: 1`，`steps` 的每一项与一个决策点对应。参见[换波操作文件](../../../core/battle/examples/team-waves-operations.json)和[复活操作文件](../../../core/battle/examples/revival-budget-operations.json)。

| 字段 | 用法 |
| --- | --- |
| `point` | 填 `normal_turn`、`extra_turn` 或 `insert`，与会话给出的窗口一致。 |
| `choice` | 选招填 `{"kind":"action","actor":"fighter","action":"skill"}`。每个放弃的插入窗口填 `{"kind":"skip"}`，包括合法列表为空的窗口。`skip` 只接受 `kind` 字段。 |
| `expect` | 可选列表，按填写顺序检查。每项填写 `offset`、`kind`、`actor`、`action` 和 `target`。`offset` 从本步首条事件数起，第一条为 `0`。`action`、`target` 无值时可省略。填写 `amount` 时必须给出数值，按事件自身的单位精确核对。只核对事件身份时省略 `amount`。 |

例如，复活罚时事件 `revival_av_penalty` 的 `amount: 25` 表示实际扣除 25 AV。生命减少量用 `hp_loss` 核对，名义伤害用 `damage` 核对。填入数值后，回放在对应事件缺少数值或数值不一致时报错，给出步骤、事件和预期与实际值。录像只显示舍入结果时，可以保留原读数并另按游戏显示精度对照报告。

报告保留常规战斗字段和 `events`，另有 `steps`，每步包含决策编号、波次、提交前观测、合法列表和选择。用 `[event_start, event_after_submit)` 查看提交期间的事件，用 `[event_after_submit, event_end)` 查看推进到下一决策或终局期间的事件。提交终结技登记入队。实际扣能和执行发生在它获准进入回合的推进阶段；队首可能先产生其他决策，已登记的请求也可能因战斗状态变化而失效。`expect` 的偏移以该事件实际出现的步骤为准。同一 AV 上的事件按索引区分。首个决策前的开战事件保留在全局 `events` 中。每步后的状态参照下一步的 `observation`，最后一步参照报告中的终局状态。

回放按场景规则决定实际目标。`expect` 对照运行产生的事件。脚本提前结束、终局后多给操作、非法选择、事件不符或记录截断会返回错误。对局受动作上限等保护规则终止时，报告的 `outcome` 仍为 `incomplete`。

---

## 2. 场景输入与输出格式规范

### 2.1 核心输入结构 (`Scenario`)

使用 [`duel.json`](../../../core/battle/examples/duel.json) 作为可运行的输入模板。正式字段由 [`Scenario`](../../../core/battle/src/model/scenario.rs) 定义，包含 `max_av`、`max_actions`、双方共享战技点、我方单位、敌方波次和 `mechanics`。开发文件中的 `schema_version`、`ruleset`（样例标为 `battle_v2`）等元数据由 [`ScenarioFile`](../../../core/battle/src/diagnostics/mod.rs) 在构造 `Scenario` 前移除。组装方负责保证静态数值和规则引用有效，`inspect` 检查 JSON 和字段结构。

### 2.2 核心机制字段 (`mechanics`) 说明

- `cycles`：各轮次（Cycle）的 AV 额度列表。组装方保证额度之和等于 `max_av`，列表为空时使用单轮 `max_av`。
- `ally_damage_factor` 是我方直接伤害的可选乘数。Rust `Mechanics::default()` 与省略此字段的开发 JSON 均取 `1.0`。填入有限非负数，`0.0` 表示该次直接伤害量为零。我方直接伤害命中存活目标时，在护盾和生命结算前乘算一次。敌方伤害使用 `1.0`。用于命中的值为负数或非有限值时返回 `ally_damage_factor must be finite and non-negative`。
- `queue_capacity`：有限敌人队列的在场槽位数。启用时，组装方将所有敌人按入场顺序放入唯一一波 `waves[0]`。敌人阵亡并结束当前 Action 后，引擎依序补足空槽。
- `triggers`: 战局全局或单位级事件监听器（`BattleStart`、`WaveStart`、`Hit`、`Break`、`Death`、`TurnEnd` 等）。

#### 普通虚数击破伤害

在动作或事件监听器中加入 `imaginary_toughness`，并给可能被击破的单位提供 `mechanics.max_toughness`。该表以单位 ID 为键，`mechanics.toughness` 仍填写当前韧性。削韧量、当前韧性、最大韧性和 `stance_unit` 使用同一单位。例如目标最大韧性为 120 时，`stance_unit` 为 10 的计算与最大韧性 360、`stance_unit` 为 30 相同。施加虚数削韧时，当前韧性须为有限非负数，并且小于或等于已配置的最大韧性。

```json
{
  "type": "imaginary_toughness",
  "amount": 10,
  "target": "all_enemies",
  "break_damage": {
    "level_multiplier": 3767.5533,
    "break_effect": 1.0,
    "stance_unit": 10
  }
}
```

示例采用 80 级参考倍率与 100% 击破特攻。组装方填入伤害来源对应的 `level_multiplier` 和 `break_effect`。目标有 `element_profiles` 条目时须填虚数抗性，省略目标条目时使用 `stats.resistance`。

存活目标的当前韧性从正数降至零时，引擎先计算伤害，再记录 `break` 并处理其监听器。目标在同一次存活中才继续结算 `break_damage`，由现有护盾、扣血和死亡逻辑处理。普通 `toughness` 效果继续只削韧并派发击破事件。回放用 `break_damage` 核对击破伤害量，用 `hp_loss` 核对实际扣血。击破伤害无普通命中事件或暴击判定。公式核对和实机待确认项见[普通虚数击破验证](validation.md#普通虚数击破)。

#### 首波独立战斗事件

`mechanics.opening_battle_event` 可配置一次首波自动插入。用全场唯一的 `id` 记录来源，例如开拓银轨的 `"62603"`，填入已组装的 `stats` 和一个零费用、零收益的 `follow_up` 动作。把各段效果按执行顺序放入 `action.effects`。首波的开战与入波监听器处理完毕后登记事件，它只在首波执行。

事件 ID 出现在诊断的请求和伤害来源中。事件只通过预登记动作参与战斗，生命、行动轴和策略操作由参战单位维护。`follow_up` 用于复用队列，事件以独立插入命中 `insert` 默认结算。普通追击的动作条件用于角色自身动作。伤害使用显式面板及效果参数，当前我方直伤占位系数在这条路径上取中性值。首波时序的验收方法和列车数值的实机待确认项见[开拓银轨开场事件验证](validation.md#开拓银轨开场事件)。

#### 传统召唤物

`mechanics.traditional_summons` 为开战时仍存活的主人配置召唤物，召唤物在 `BattleStart` 前入场。每条配置包含非空且全场唯一的 `id`、开战我方名单中的主人 `owner`、有效面板 `stats` 以及一个自动执行的 `action`。`stats.speed` 和 `stats.max_hp` 均须为有限正数。`action.kind` 设为 `follow_up`，`sp_cost`、`sp_gain`、`energy_cost` 和 `energy_gain` 均填 `0`。

用 `elemental_damage` 表达元素伤害，并将 `attack_type` 设为 `pursued`，命中时会带上追击类型供监听器筛选。`mechanics.element_profiles` 以单位 ID 为键，每个可能使用的元素都需要召唤物对应的 `damage_bonus` 和每个可能命中目标对应的 `resistance`。

以下 PowerShell 命令从仓库根目录运行。它读取[开战召唤物合成场景](../../../core/battle/examples/opening-summon.json)，在临时文件中将原有伤害替换为风属性追击，然后以种子 42 运行并取出召唤物的伤害事件。

```powershell
$scenario = Get-Content -Raw core/battle/examples/opening-summon.json | ConvertFrom-Json
$scenario.mechanics.traditional_summons[0].action.effects = @(
    @{
        type = 'elemental_damage'
        scale = 'attack'
        ratio = @{ op = 'constant'; value = 0 }
        flat = @{ op = 'constant'; value = 40 }
        can_crit = $false
        target = 'enemy_lowest_hp'
        element = @{ selection = 'fixed'; element = 'wind' }
        attack_type = 'pursued'
    }
)
$scenario.mechanics | Add-Member -NotePropertyName element_profiles -NotePropertyValue @{
    summon = @{ damage_bonus = @{ wind = 0 }; resistance = @{ wind = 0 } }
    enemy = @{ damage_bonus = @{ wind = 0 }; resistance = @{ wind = 0 } }
}

$scenarioPath = [IO.Path]::GetTempFileName()
try {
    [IO.File]::WriteAllText($scenarioPath, ($scenario | ConvertTo-Json -Depth 30))
    $reportJson = cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate $scenarioPath 42
    if ($LASTEXITCODE -ne 0) { throw "hcwa-battle exited with $LASTEXITCODE" }
    $report = ($reportJson -join [Environment]::NewLine) | ConvertFrom-Json
    $report.events |
        Where-Object { $_.kind -eq 'damage' -and $_.actor -eq 'summon' } |
        Select-Object kind, actor, target, av, amount, attack_type |
        ConvertTo-Json
} finally {
    Remove-Item -LiteralPath $scenarioPath
}
```

主人在 AV 100 行动，随后命令输出一条召唤物伤害事件。

```json
{
  "kind": "damage",
  "actor": "summon",
  "target": "enemy",
  "av": 125.0,
  "amount": 40.0,
  "attack_type": "pursued"
}
```

召唤物的回合时序及输出字段见[调度设计](design.md#23-传统召唤物回合)和[分步观测](stepping.md#22-局面观测-observation)。

### 2.3 输出结果结构 (`BattleResult` 与 `BattleReport`)

- **运行时原生结果 (`BattleResult`)**：
  纯内存核心直接产出的对局结果，包含：
  - `outcome`：`Victory`（胜利）、`Defeat`（失败）、`Draw`（双方全灭）或 `Incomplete`（未完成）。
  - `termination`：具体终止原因，包括敌方全灭、我方全灭、双方全灭、轮次耗尽、达到动作上限和调用方取消。
  - `elapsed_av` 与 `remaining_av`: 实际消耗 AV 与剩余总预算。
  - `actions`: 本场已完成的完整动作总数。
  - `units`：已入场单位的终态快照，保留阵亡记录及生命、护盾、能量、剩余行动距离、速度和状态等字段。

- **CLI 诊断报告 (`BattleReport`，需 `diagnostics` 特性)**：
  CLI 工具 `hcwa-battle` 导出的 JSON 诊断包装，在 `BattleResult` 的基础上增加审计字段：
  - `provenance` 与 `scenario_sha256`：输入来源及对重新序列化的完整 JSON 计算的 SHA-256 指纹。
  - `seed`: 64 位无符号整型种子的十进制无损字符串。
  - `events` 与 `trace_truncated`：CLI 的 `simulate` 记录有序事件，并标明记录是否达到上限。
  - `settlement`：[那刻夏开发样例](anaxa-scenario.md)使用的临时诊断结果。

---

## 3. Rust 库调用示例

### 3.1 同步快速模拟

默认构建直接运行组装好的场景：

```rust
use battle_core::engine::simulate;
use battle_core::model::Scenario;

fn run_single_battle(scenario: &Scenario, seed: u64) -> Result<(), String> {
    // 纯内存核心直接执行单局模拟
    let result = simulate(scenario, seed)?;

    println!("对局结果: {:?}, 动作数: {}", result.outcome, result.actions);
    println!("消耗 AV: {:.2}", result.elapsed_av);
    Ok(())
}
```

启用 `diagnostics` 特性时，支持从 JSON 文件读取并记录逐事件回放（trace）：

```rust
use battle_core::diagnostics::{simulate as diag_simulate, ScenarioFile};

fn run_diagnostic_battle(scenario_json: &str, seed: u64) -> Result<(), String> {
    let file = ScenarioFile::from_slice(scenario_json.as_bytes())?;
    // trace = true 记录详细事件流
    let result = diag_simulate(&file.battle, seed, true)?;
    let report = file.report(result, seed);

    println!("对局结果: {:?}, 终止原因: {:?}", report.result.outcome, report.result.termination);
    Ok(())
}
```

### 3.2 外部策略分步交互 (`BattleSession`)

对于需要逐步观察战局并执行策略的场景，使用 [`BattleSession`](stepping.md)：

```rust
use battle_core::engine::{BattleSession, BattleStep};
use battle_core::model::Scenario;

fn run_interactive_battle(scenario: &Scenario, seed: u64) -> Result<(), String> {
    // 初始化会话（默认纯内存模式只需传入场景引用与种子）
    let mut session = BattleSession::new(scenario, seed)?;

    loop {
        let (id, choice) = match session.advance()? {
            BattleStep::Decision(decision) => (decision.id, decision.legal.first().cloned()),
            BattleStep::Finished(result) => {
                println!("战斗结束，胜负状态: {:?}", result.outcome);
                break;
            }
        };
        session.submit(id, choice)?;
    }

    Ok(())
}
```

---

## 4. 示例场景资源

场景文件位于 [`core/battle/examples/`](../../../core/battle/examples/)：

- [`duel.json`](../../../core/battle/examples/duel.json)：人工单挑场景，展示基础伤害、暴击和行动顺序。
- [`team-waves.json`](../../../core/battle/examples/team-waves.json)：多角色队伍对战两波敌人，展示波次衔接与战技点共享。
- [`revival-budget.json`](../../../core/battle/examples/revival-budget.json)：展示复活时扣减当前轮次额度。
- [传统召唤物](#传统召唤物)：展示开战入场、独立回合和元素追击的诊断步骤。
- [`anaxa-s2-full-battle.json`](../../../core/battle/examples/anaxa-s2-full-battle.json)：那刻夏 S2 的部分技能链与有限队列开发样例，见[样例说明](anaxa-scenario.md)。
