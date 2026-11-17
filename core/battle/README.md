# 战斗模拟核心库 (Battle Core)

`battle-core` 是崩铁货币战争 AI 系统的独立战斗模拟核心，提供纯 Rust 编写的确定性战斗规则演算、蒙特卡洛批次评估以及命令行工具 `hcwa-battle`。

本模块不依赖游戏客户端与 UI 框架，在架构上实现了**纯内存轻量运行时 (Core Runtime)** 与**诊断及序列化系统 (Diagnostics)** 的解耦：默认编译为零额外反序列化开销的高性能纯内存核心；在启用 `diagnostics` 特性时提供场景 JSON 校验、序列化输出与整局血量（Run HP）结算诊断。

详细的系统设计、时序机制与分步 API 请查阅模块技术文档：
- [系统架构与逻辑设计](../../docs/modules/battle-simulation/design.md)
- [外部选招与分步会话接口](../../docs/modules/battle-simulation/stepping.md)
- [测试与回归验证指南](../../docs/modules/battle-simulation/validation.md)
- [光锥效果数据](../../docs/game-data/light-cone-effects.md)
- [遗器套装效果数据](../../docs/game-data/relic-set-effects.md)

---

## 1. 命令行工具 (`hcwa-battle`)

Crate 内置命令行工具 `hcwa-battle`（位于 `src/main.rs`），要求 `diagnostics` 特性，用于离线校验场景输入、单局模拟与批量蒙特卡洛评估。

### 1.1 子命令手册

从仓库根目录执行（统一使用锁定工具链）：

```powershell
# 1. 检查并校验场景输入语法、数值合法性及计算场景指纹
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- inspect <SCENARIO.json>

# 2. 单局确定性模拟（参数：场景文件 随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- simulate <SCENARIO.json> <SEED_U64>

# 3. 蒙特卡洛批次评估（参数：场景文件 试验场次 主随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- batch <SCENARIO.json> <TRIALS> <MASTER_SEED_U64>
```

### 1.2 退出码与错误输出

- **退出码 0**：执行成功，标准输出（stdout）打印格式化的 JSON 结果。
- **退出码 2**：输入校验失败、参数错误或运行时异常，错误详情写入标准错误（stderr）。
- 建议将大量批次运行结果重定向至本地忽略目录，如 `runs/result.json`。

---

## 2. 场景输入与输出格式规范

### 2.1 核心输入结构 (`Scenario`)

场景文件为 JSON 格式，顶层契约版本需声明为 `"battle_v2"`：

```json
{
  "version": "battle_v2",
  "provenance": {
    "game_version": "4.7",
    "description": "标准场景示例"
  },
  "max_av": 1000.0,
  "action_limit": 200,
  "allies": [
    {
      "id": "player_char_1",
      "stats": {
        "level": 80,
        "max_hp": 3500.0,
        "atk": 1200.0,
        "def": 800.0,
        "speed": 134.0,
        "crit_rate": 0.70,
        "crit_dmg": 1.40,
        "dmg_bonus": 0.30
      },
      "initial_hp": 3500.0,
      "initial_energy": 50.0,
      "energy_limit": 120.0,
      "actions": [ /* 技能列表：普攻、战技、终结技 */ ]
    }
  ],
  "waves": [
    [
      {
        "id": "enemy_1",
        "stats": { /* 敌人有效面板 */ },
        "initial_hp": 50000.0,
        "actions": [ /* 敌人行动配置 */ ]
      }
    ]
  ],
  "ally_skill_points": {
    "initial": 3,
    "maximum": 5
  },
  "mechanics": {
    "cycles": [150.0, 100.0, 100.0],
    "ally_damage_factor": 0.3,
    "triggers": []
  }
}
```

### 2.2 核心机制字段 (`mechanics`) 说明

- `cycles`: 各轮次（Cycle）的 AV 额度列表。列表之和必须严格等于 `max_av`；省略时默认为单轮 `max_av`。
- `ally_damage_factor`: 我方伤害全局系数（在通用伤害公式结算后统一乘算一次；货币战争特定环境可显式声明为 `0.3`，默认为 `1.0`）。
- `queue_capacity`: 可选敌人有限队列容量（1..32）。启用后 `waves[0]` 作为有序待入场候选池，当场上敌人被彻底消灭且动作完全收尾后依序补入。
- `settlement`: 可选整局血量结算配置（如 `weighted_kills_flat_penalty_v1`），声明每个敌人实例的击杀权重、通关进度门槛、失败扣血量与战后生命恢复量。
- `triggers`: 战局全局或单位级事件监听器（`BattleStart`、`WaveStart`、`Hit`、`Break`、`Death`、`TurnEnd` 等）。

### 2.3 输出结果结构 (`BattleResult` 与 `BattleReport`)

- **运行时原生结果 (`BattleResult`)**：
  纯内存核心直接产出的对局结果，包含：
  - `outcome`: 战斗胜负状态（`Outcome::Victory` 胜利、`Outcome::Defeat` 失败、`Outcome::Incomplete` 安全保护或取消停机）。
  - `termination`: 具体终止原因（`Termination::EnemiesDefeated` 敌方全灭、`Termination::AlliesDefeated` 我方全灭、`Termination::AvBudgetExhausted` 轮次耗尽、`Termination::ActionLimit` 步数超限、`Termination::Cancelled` 主动取消）。
  - `elapsed_av` 与 `remaining_av`: 实际消耗 AV 与剩余总预算。
  - `actions`: 本场已完成的完整动作总数。
  - `units`: 战斗结束时各在场单位的终态快照（包含实时 HP、护盾、能量、已行动距离、速度与持有的 Modifier 状态）。

- **CLI 诊断报告 (`BattleReport`，需 `diagnostics` 特性)**：
  CLI 工具 `hcwa-battle` 导出的 JSON 诊断包装，在 `BattleResult` 的基础上增加审计字段：
  - `provenance` 与 `scenario_sha256`: 输入来源说明与 JSON 全配置的 SHA-256 指纹。
  - `seed`: 64 位无符号整型种子的十进制无损字符串。
  - `settlement`: 可选整局血量结算详情（击杀权重进度、达标状态、扣血与恢复、结算后整局血量）。
  - `events`: （若开启回放）有序事件轨迹列表，详细记录每次伤害跳字、暴击、护盾吸收与生命值变动。

---

## 3. Rust 库调用示例

### 3.1 同步快速模拟

在默认核心下（纯内存结构，零反序列化开销）：

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

对于需要逐步观察战局并执行 AI 策略的场景，使用 [`BattleSession`](../../docs/modules/battle-simulation/stepping.md)：

```rust
use battle_core::engine::session::{BattleSession, BattleStep};
use battle_core::model::Scenario;

fn run_interactive_battle(scenario: &Scenario, seed: u64) -> Result<(), String> {
    // 初始化会话（默认纯内存模式只需传入场景引用与种子）
    let mut session = BattleSession::new(scenario, seed)?;

    loop {
        let (id, choice) = match session.advance()? {
            BattleStep::Decision(decision) => {
                // 读取 observation 观察当前战况
                let obs = &decision.observation;
                println!("当前 AV: {:.1}, 存活单位数: {}", obs.now, obs.units.len());

                // 从合法操作中做出选择（示例选取第一个合法动作）
                (decision.id, decision.legal.first().copied())
            }
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

Crate 下提供了若干经过验证的基础样例文件，位于 [`examples/`](examples/) 目录：

- [`examples/duel.json`](examples/duel.json)：人工单挑场景，展示基础伤害、暴击与速度拉条逻辑。
- [`examples/team-waves.json`](examples/team-waves.json)：多角色队伍对战两波敌人，展示波次衔接与战技点共享。
- [`examples/revival-budget.json`](examples/revival-budget.json)：展示复活扣减轮次余量罚时。
- [`examples/anaxa-s2-full-battle.json`](examples/anaxa-s2-full-battle.json)：那刻夏 S2 机制的完整技能链与有限队列端到端结算演示。
