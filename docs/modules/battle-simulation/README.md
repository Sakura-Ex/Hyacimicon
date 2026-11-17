# 基础战斗模拟 (Battle Simulation)

`core/battle` 维护确定性、显式输入的离线战斗模拟核心库 `battle-core` 以及命令行工具 `hcwa-battle`。

本模块专注纯粹的战斗规则与时序结算：接收调用方（如整局环境 `core/run` 或离线分析工具）组装完毕的有效面板、动作定义与规则参数，执行确定性战斗演算，输出对局结果、单位快照与事件序列；或通过分步会话接口为外部策略、AI 决策模型提供合法动作空间与局面观测。

---

## 目录导航

| 文档 | 核心内容 |
| --- | --- |
| [系统架构与逻辑设计](design.md) | 模块分层架构、完整时序状态机、伤害与效果结算管线、核心类型系统 |
| [分步会话与外部决策](stepping.md) | 推理与训练共用的 `BattleSession` 接口、推进阶段、合法动作掩码与时序边界 |
| [测试与回归验证](validation.md) | 测试架构、确定性随机与浮点往返保障、用例分布与自动化回归执行方式 |
| [Crate 与 CLI 使用手册](../../../core/battle/README.md) | `battle-core` 库依赖、`hcwa-battle` 命令行子命令、输入 JSON Schema 规范与样例 |

---

## 核心特性与能力矩阵

| 维度 | 能力说明 |
| --- | --- |
| **行动时序调度** | 基于“剩余行动距离 / 当前速度”推进时间；单位常规行动（Turn）结束后重排；速度变化保持当前剩余距离；Cycle 倒计时拥有独立 AV 额度，支持多轮不同额度配置。 |
| **动作与资源机制** | 共享战技点（队伍共享）与个体能量系统；支持常规回合、终结技（Ultimate）、额外回合（Extra Turn）与追加攻击（Follow-up）；终结技排队进入时先校验资源并扣能；行动插入不推进自然 AV 时钟。 |
| **效果与数值管线** | 支持七元素独立面板；实现基础数值、增伤、减伤、易伤、防御、抗性及虚弱等多乘区计算；逐命中（Hit-by-hit）独立暴击采样；优先护盾吸收再扣减 HP；支持韧性扣减与击破事件分派。 |
| **状态与变量作用域** | 状态（Modifier）支持按来源独立叠层、刷新及到期结算；支持弱点附加与自然弱点动态合并；支持单位级持久变量及 Ability / TaskTemplate 临时调用帧栈。 |
| **波次与有限队列** | 支持多波次战斗；支持 `waves[0]` 的有限敌人槽位补充机制（`finite_queue_after_action_v1`），在前序单位阵亡且动作彻底结算后有序补入新敌人。 |
| **整局血量结算** | 支持单场战斗的可选整局血量结算策略（`weighted_kills_flat_penalty_v1`），依据敌人击败进度权重、达标门槛及失败罚血计算战后整局血量变化。 |
| **外部决策交互** | `BattleSession` 提供非阻塞的分步推进；在需要外部决策时暂停并产出观测与合法动作掩码，支持自动策略（`Automatic`）与外部策略/模型通过统一接口提交选择。 |
| **批次评估与分析** | 支持基于主种子的确定性多局批次模拟；独立分离目标、暴击与元素三条随机流；支持胜率频率统计及 95% Wilson 置信区间评估；任意单局可通过对应种子精确重放。 |

---

## 核心规则契约

1. **规则版本**：当前版本契约统一采用 `battle_v2`。
2. **两类独立的终局触发**：
   - **完全淘汰**：我方全灭或敌方全部阵亡（包括有限队列中的待入场敌人均被击败），立即完成终局结算。
   - **轮次倒计时耗尽**：总 AV 预算归零且最终 Cycle 到期。当 AV 额度归零时，已进入的单位回合（Turn）仍会完整收尾，随后执行到期 Cycle 结算；若仍未清场则判为时限耗尽。
3. **安全停止与未完成状态**：
   - 支持行动步数上限（`action_limit`）与外部异步取消（Cancellation）。
   - 触发保护或取消时，战斗标记为 `Incomplete`，保留当时局面快照，不作为战斗失败计入通关分母。
4. **确定性与精度要求**：
   - 种子为 64 位无符号整型（Rust `u64`），序列化为十进制字符串传输，避免外部前端或中间件的浮点精度截断。
   - 场景支持浮点往返序列化（`float_roundtrip`），确保相同输入在多次解析重读后保持 SHA-256 指纹及边界行为严格一致。

---

## 快速上手

从仓库根目录执行以下命令（Rust 工具链与依赖由根目录 `rust-toolchain.toml` 及 `Cargo.lock` 统一锁定）：

```powershell
# 运行全部测试套件
cargo test -p battle-core --locked

# 校验场景输入格式与有效性
cargo run -p battle-core --release --locked -- validate core/battle/examples/duel.json

# 单局模拟（参数：场景文件 随机种子）
cargo run -p battle-core --release --locked -- simulate core/battle/examples/team-waves.json 42

# 批次模拟（参数：场景文件 运行场次 主随机种子）
cargo run -p battle-core --release --locked -- batch core/battle/examples/duel.json 10000 20261114

# 分步交互示例（展示外部决策循环）
cargo run -p battle-core --example step_battle --locked -- core/battle/examples/duel.json 42
```

内置样例说明：
- `examples/duel.json`：人工构建的 1v1 纯数值对决，展示基础攻防与暴击逻辑。
- `examples/team-waves.json`：展示多角色队伍在双波次敌人下的战技点共享与波次刷新。
- `examples/revival-budget.json`：展示单位复活机制与 Cycle 额度扣减罚时。
- `examples/anaxa-s2-full-battle.json`：端到端完整技能链与有限队列结算的综合演示样例（详细说明参见 [样例说明](../../../core/battle/ANAXA_SCENARIO.md)）。
