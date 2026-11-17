# 战斗模拟架构与逻辑设计

本文档阐述 `core/battle` 战斗模拟核心的设计架构、核心运行时与诊断分离机制、时序调度算法以及伤害与效果计算管线。

---

## 1. 架构分层与双层设计 (Runtime vs. Diagnostics)

`battle-core` 遵循单向依赖与职责明确的分层设计，并在架构上实现了**纯内存轻量运行时 (Core Runtime)** 与**诊断及序列化系统 (Diagnostics)** 的解耦：

```mermaid
flowchart TD
    subgraph DiagnosticsLayer["诊断与接入层 (feature = 'diagnostics')"]
        RawJSON["JSON 场景输入"] --> ParseFile["ScenarioFile::from_slice 解析与指纹提取"]
        ParseFile --> ScenarioFile["ScenarioFile (包含 battle, provenance, settlement, fingerprint)"]
        DiagSettlement["diagnostics::settlement (整局血量结算诊断)"]
        CLI["hcwa-battle CLI 工具"]
    end

    subgraph CoreRuntime["纯内存核心运行时 (Default Feature: 无外部序列化依赖)"]
        subgraph InputTypes["输入与接口抽象"]
            ScenarioModel["Scenario (纯内存对局配置)"]
            SessionAPI["BattleSession (分步外部交互会话)"]
            DecisionModel["Decision / Observation / Operation"]
        end

        subgraph Engine["执行引擎层 (Engine)"]
            State["State (对局活跃状态与队列)"]
            Timeline["Timeline (距离/速度/时钟调度)"]
            Operations["Operations (合法操作与资源校验)"]
            Effects["Effects (有序效果树执行器)"]
        end

        subgraph Mechanics["底层规则机制层 (Mechanics)"]
            Math["math (多乘区伤害/治疗/护盾公式与安全防守)"]
            Modifiers["modifiers (状态叠层与计时)"]
            Elements["elements (七元素面板与抗性)"]
            Scope["dynamic_scope (调用帧栈与局部变量)"]
        end

        subgraph Output["原生计算结果"]
            NativeResult["BattleResult / BatchResult"]
        end
    end

    ScenarioFile -->|解构获得 .battle| ScenarioModel
    ScenarioModel --> State
    SessionAPI <--> State
    State --> Timeline
    State --> Operations
    Operations --> Effects
    Effects --> Math
    Effects --> Modifiers
    Effects --> Elements
    Effects --> Scope
    State --> NativeResult
    NativeResult -.->|report 方法组合包装| DiagSettlement
```

### 1.1 双层解耦设计优势

- **Core Runtime（默认轻量依赖）**：
  默认无需引入外部序列化库，所有数据结构在纯内存中直接构造、更新与交互。专为后续大规模强化学习采样、MCTS 树搜索及高性能批次评估设计，避免高频调用中反复序列化与反序列化的 CPU 开销。
- **Diagnostics（`feature = ["diagnostics"]`）**：
  按需启用 `serde` 与 `serde_json` 依赖，提供文件 IO、JSON 场景解析、人类可读诊断事件导出以及整局血量（Run HP）结算诊断。命令行工具 `hcwa-battle` 与示例 `step_battle` 均基于此特性构建。

### 1.2 模块职责划分

| 模块目录 | 相对路径 / 命名空间 | 核心职责 |
| --- | --- | --- |
| `src/model/` | [`battle_core::model`](../../../core/battle/src/model/mod.rs) | 纯数据结构定义。包含对局场景配置、单位、动作定义、事件类型及原生结果模型。在 `diagnostics` 特性下提供 Serde 序列化标注。 |
| `src/engine/` | [`battle_core::engine`](../../../core/battle/src/engine/mod.rs) | 战斗执行主循环与状态维护。包含会话状态机、单位队列管理、行动时序推进、动作派发与目标选择。 |
| `src/mechanics/` | [`battle_core::mechanics`](../../../core/battle/src/mechanics/mod.rs) | 无状态或局部状态的通用数学计算与规则子系统。包含多乘区伤害算式、属性状态叠层、七元素属性与表达式求值。 |
| `src/decision.rs` | [`battle_core::decision`](../../../core/battle/src/decision.rs) | 外部决策与策略抽象。提供局面观测快照、合法动作空间定义及策略 Trait。 |
| `src/batch.rs` | [`battle_core::batch`](../../../core/battle/src/batch.rs) | 蒙特卡洛多局批次运行器，管理独立随机种子派生与胜率统计评估。 |
| `src/diagnostics/` | [`battle_core::diagnostics`](../../../core/battle/src/diagnostics/mod.rs) | （可选特性）提供场景文件读取（`ScenarioFile`）、诊断报告包装（`BattleReport`）与整局血量结算诊断（`settlement`）。 |

---

## 2. 核心执行流程与时序调度

单场战斗由 [`BattleSession`](../../../core/battle/src/engine/session.rs) 驱动。引擎主循环执行以下关键阶段：

```mermaid
stateDiagram-v2
    [*] --> Ready: 读取 Scenario 并初始化 State
    Ready --> CheckTerminal: 检查终局淘汰与波次切换
    CheckTerminal --> ProcessInsert: 优先处理排队中的插入动作 (终结技/额外回合/追击)
    ProcessInsert --> AdvanceTurn: 无插入，推进常规时钟至最近单位
    AdvanceTurn --> DecisionNeeded: 到达决策点，生成 Observation 与合法 Legal Operations
    DecisionNeeded --> ExecuteAction: 外部或内部策略提交选择
    ExecuteAction --> ApplyEffects: 执行动作效果树并分派事件
    ApplyEffects --> RefillQueue: 动作彻底结束，补充有限敌人队列
    RefillQueue --> CheckTerminal
    CheckTerminal --> Finished: 全灭/清场/Cycle超时/取消/达步数上限
```

### 2.1 行动距离与时间推进算法

行动轴基于经典“行动距离 / 当前速度”模型推进，调度规则严格保证确定性：

1. **基本单位**：每个存活单位持有 `remaining_distance`（初始基础值为 10,000）和当前速度 `speed`。
2. **行动等待时间（Action Value, AV）**：
   $$\Delta \text{AV} = \frac{\text{remaining\_distance}}{\text{speed}}$$
3. **时间步推进**：
   - 选取所有存活单位中等待时间最小的单位，其等待时间为步长 $\Delta t$。
   - 所有存活单位的行动距离同步扣减：
     $$\text{remaining\_distance} \leftarrow \text{remaining\_distance} - \Delta t \times \text{speed}$$
   - 全局已流逝时间累计 $\text{now} \leftarrow \text{now} + \Delta t$。
4. **变速与拉条规则**：
   - **动态变速**：修改当前速度 `speed` 时，单位已推进到的 `remaining_distance` 保持不变，仅更新后续等待时间。
   - **行动提前 / 推条**：`advance` 效果直接修改当前距离；`advance_next` 则记录下一次行动的初始距离偏移。
   - 当前正在执行普通行动的单位，其距离保持在 0，直至 `TurnEnd` 后才重新分配基础距离并重排。

### 2.2 插入动作与回合优先级

行动窗口按照以下层级调度：

- **终结技插入（Ultimate）**：可在常规回合动作的间隔插入。提交插入仅表示预排队；在终结技实际获得执行权切入 Turn 时，引擎再次校验能量并正式扣除费用。
- **追加攻击（Follow-up）**：作为直接 Action 执行，不产生独立的 `TurnStart` 与 `TurnEnd` 回合边界。
- **额外回合（Extra Turn）**：在独立回合开始（`TurnStart`）后发起选招，不推进自然 AV 时钟。
- **同刻平局判定**：当单位行动与 Cycle 到期发生同刻时，支持通过显式策略参数 `unit_before_cycle` 控制优先处理单位回合还是优先倒计时。

---

## 3. 伤害与数值结算管线 (Damage Pipeline)

每次攻击命中的数值计算均在确定性规则下执行，完整结算流程如下：

```mermaid
flowchart LR
    Base[基础数值 Scaling] --> Multipliers[乘区展开: 增伤/易伤/防御/抗性/虚弱]
    Multipliers --> SafetyCheck[动态属性防守检查]
    SafetyCheck --> AllyFactor[我方伤害系数 ally_damage_factor]
    AllyFactor --> CritRNG[逐命中独立暴击判定]
    CritRNG --> Shield[护盾优先吸收]
    Shield --> HP[扣减剩余生命值]
    HP --> Events[分派 Hit/Break/Death 事件]
```

### 3.1 伤害乘区公式与安全性防守

每次命中（Hit）的理论伤害计算公式如下：

$$\text{Damage} = \text{Base} \times (1 + \text{DmgBonus}) \times (1 + \text{Vulnerability}) \times \text{DefMult} \times \text{ResMult} \times \text{BrokenMult} \times \text{WeakenMult} \times \text{Factor}$$

1. **基础数值 (Base)**：由攻击者面板（攻击力、防御力或生命上限比例）结合动作配置确定。
2. **增伤乘区 (DmgBonus)**：全局增伤与当前命中对应元素的属性增伤之和，加上实时活动的 Modifier 增益。
   - **安全性防守**：综合增伤乘率不得小于零（即要求 `damage_bonus >= -1.0`）；暴击伤害加成不得为负数（`crit_damage >= 0.0`）。
3. **易伤乘区 (Vulnerability)**：受击者身上的易伤状态总和。
4. **防御乘区 (DefMult)**：
   $$\text{DefMult} = \frac{\text{attacker\_level} \times 10 + 200}{(\text{attacker\_level} \times 10 + 200) + \text{defender\_def} \times (1 - \text{def\_ignore})}$$
5. **抗性乘区 (ResMult)**：根据目标对应元素的抗性减去减抗属性计算，范围严格限制在 $[-1.0, 0.9]$。
6. **击破状态乘区 (BrokenMult)**：目标处于弱点击破状态时通常取 1.0，未击破状态通常取 0.9。
7. **我方伤害系数 (Factor)**：由场景显式声明的 `mechanics.ally_damage_factor`，默认值为 1.0；特定玩法下可显式配置（例如当前货币战争实测为 0.3），在公共结算点统一乘算一次。

### 3.2 护盾吸收与生命扣减

- 计算出的实际伤害优先扣减受击者当前持有的 `shield`。
- 护盾不足或耗尽时，超出部分的伤害扣减 `current_hp`。
- 仅当某次命中的扣减使得正生命值归零时，才分派 `Death` 事件并触发可能的复活判定（`Revival`）。

---

## 4. 核心类型规约

### 4.1 输入配置模型 ([`model`](../../../core/battle/src/model/mod.rs))

- [`Scenario`](../../../core/battle/src/model/scenario.rs)：运行时战斗配置结构体。包含队伍列表、波次定义、AV 预算、行动步数上限及底层机制配置（`mechanics`）。
- [`Unit`](../../../core/battle/src/model/unit.rs)：单位基础定义，包含静态面板 [`Stats`](../../../core/battle/src/model/unit.rs)、初始能量/生命、复活配置及技能动作列表。
- [`Action`](../../../core/battle/src/model/action.rs)：动作单元，包含动作类型（普攻/战技/终结技/追击）、费用与能量收益、以及树状嵌套的有序效果列表（`Vec<Effect>`）。
- [`Effect`](../../../core/battle/src/model/action.rs)：具体效果枚举，包含伤害、治疗、加盾、修改变量、拉条、施加状态、条件分支（`Conditional`）与循环（`Repeat`）。
- [`ScenarioFile`](../../../core/battle/src/diagnostics/mod.rs)：（仅 `diagnostics` 特性）承载外围 JSON 文件的容器，包含来源描述 `provenance`、场景指纹 `fingerprint`、近似假设 `assumptions`、可选整局血量结算 `settlement` 及内部对局配置 `battle: Scenario`。

### 4.2 运行时状态模型 ([`engine::state`](../../../core/battle/src/engine/state.rs))

- [`State`](../../../core/battle/src/engine/state.rs)：单场战斗的活跃状态机。维护存活单位表、行动队列、共享战技点、时钟、监听器及分离的随机数生成流（RNG）。
- [`CombatUnit`](../../../core/battle/src/engine/state.rs)：单位在局内的活跃实例。维护动态生命值、护盾、能量、韧性、实时速度、剩余行动距离及持有的 Modifier 状态集合。
- [`Timeline`](../../../core/battle/src/engine/timeline.rs)：负责时间步计算、Cycle 额度管理及复活罚时结算。

### 4.3 决策与交互模型 ([`decision`](../../../core/battle/src/decision.rs))

- [`Decision`](../../../core/battle/src/decision.rs)：代表一个暂停待决的决策点，包含局内单调递增的 `id`、决策类型、当前完整观测及合法操作列表。
- [`Observation`](../../../core/battle/src/decision.rs)：提供给外部策略的局面只读快照，包含当前 AV、各轮次余量、双方战技点、排序后的行动轴及所有在场单位的属性快照。
- [`Operation`](../../../core/battle/src/decision.rs)：合法操作载体，标明所选单位索引与动作索引。

### 4.4 结果与统计模型

- [`BattleResult`](../../../core/battle/src/model/result.rs)：运行时原生输出。包含：
  - `outcome`: 胜负状态（`Outcome::Victory` 胜利、`Outcome::Defeat` 失败、`Outcome::Incomplete` 未完成）。
  - `termination`: 终止原因（`Termination::EnemiesDefeated` 敌方全灭、`Termination::AlliesDefeated` 我方全灭、`Termination::AvBudgetExhausted` 轮次耗尽、`Termination::ActionLimit` 步数超限、`Termination::Cancelled` 主动取消）。
  - `elapsed_av` 与 `remaining_av`: 实际消耗与剩余总 AV。
  - `actions`: 完成动作数。
  - `units`: 单位终态快照。
- [`BattleReport`](../../../core/battle/src/diagnostics/mod.rs)：（仅 `diagnostics` 特性）CLI 导出的完整审计报告，包装了 `BattleResult`，并补充了种子字符串、场景指纹、`provenance`、`settlement` 结果及详细回放事件列表（`Event`）。
- [`BatchResult`](../../../core/battle/src/batch.rs)：蒙特卡洛多局评估输出，包含胜败频次、未完成场次、95% Wilson 置信区间及逐场明细账本（Ledger）。
