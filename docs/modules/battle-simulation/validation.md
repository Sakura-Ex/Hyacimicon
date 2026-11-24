# 战斗模拟验证与测试指南

本文档阐述 `core/battle` 战斗模拟核心的自动化验证体系、测试分层、确定性保障原则以及本地回归执行方式。

---

## 1. 验证体系架构

战斗模块的正确性由多层次测试套件共同保障：

```mermaid
flowchart TD
    subgraph RustUnit["Rust 核心测试 (core/battle/tests/)"]
        Formula["手算与参考公式验证 (math, reference_math)"]
        StateTests["时序与生命周期测试 (session, runtime_control)"]
        QueueTests["队列与结算边界测试 (queue_deadline, full_battle)"]
        BatchTests["批次蒙特卡洛与种子测试 (batch)"]
    end

    subgraph ToolTests["工具与配置测试 (core/battle/tools/tests/)"]
        RawConfig["Raw Config 技能编译器解析测试"]
        RuntimeLowering["运行时效果与弱点生成测试"]
    end

    subgraph Determinism["确定性与精度保障"]
        FloatRoundtrip["浮点序列化往返精度 (float_roundtrip)"]
        SeedIsolation["三流随机数隔离 (Target / Crit / Element)"]
        ReplayBit["固定种子的结果与事件顺序"]
    end

    Formula --> Determinism
    StateTests --> Determinism
    QueueTests --> Determinism
    BatchTests --> Determinism
```

### 测试用例分布

| 测试目录 / 文件 | 覆盖范围与验证要点 |
| --- | --- |
| `tests/reference_math.rs` | 用 18 组参考值核对直接伤害乘区。 |
| `tests/session.rs` | 核对暂停期间重复读取、拒绝过期 ID 和非法操作、终结技扣能、取消及回合收尾顺序。 |
| `tests/queue_deadline.rs` | 覆盖有限敌人队列与轮次耗尽边界：动作彻底完成后补位、超时快照保全、同刻 Cycle 结算次序。 |
| `tests/runtime_control.rs`、`tests/dynamic_scopes.rs` | 核对条件分支、循环次数上限、动作内目标绑定和调用帧变量隔离。 |
| `tests/property_modifiers.rs` | 覆盖状态（Modifier）生命周期：叠层上限、按来源独立叠加、回合到期扣减、即时监听修改后续同 Action 命中数值。 |
| `tests/batch.rs` | 核对种子派生参考值、逐场复现、已完成场次的胜负统计、取消与 95% Wilson 区间。 |
| `tools/tests/` | 核对开发用原始配置转换、表达式解析、效果和弱点转换及那刻夏场景构建。 |

---

## 2. 确定性与回归原则

固定场景、种子与操作序列后，可按以下规则核对模拟结果：

1. **三流随机数隔离 (Three Independent RNG Streams)**：
   - 每场战斗基于单一输入种子，通过 `rand_chacha` 派生三条完全独立的伪随机数流：**目标选择流**、**暴击判定流**、**元素选择流**。
   - 各类抽样各自推进随机流。同一流上的抽样次数仍取决于实际执行路径。
2. **64 位种子无损传输**：
   - 随机种子在 Rust 内部为 `u64`。
   - 在 JSON 格式中序列化为十进制字符串（如 `"18446744073709551615"`），防止外部语言或前端运行时解析为 JavaScript Number 时发生精度截断。
3. **浮点往返序列化一致性**：
   - 开发 JSON 使用 `serde_json` 的 `float_roundtrip` 特性。场景指纹基于解析后重新序列化的完整 JSON，包含开发元数据。
4. **单局与批次对齐**：
   - 用批次摘要里的派生种子重新运行同一场景，可比对完成场次的胜负、终止原因、AV 和动作数。逐事件回放由单局 `simulate` 输出。

---

## 3. 本地验证执行方式

### 3.1 核心 Rust 测试与代码质量检查

在仓库根目录下运行：

```powershell
# 1. 运行默认构建的核心流程测试
cargo test -p battle-core --locked

# 2. 运行含 diagnostics 的战斗测试
cargo test -p battle-core --all-features --locked

# 3. 执行严格静态代码检查（禁止任何 Warning）
cargo clippy -p battle-core --all-targets --all-features --locked -- -D warnings

# 4. 检查代码格式
cargo fmt --all -- --check
```

### 3.2 Python 配置与工具测试

在 `core/battle/` 目录下运行：

```powershell
python -m unittest discover -s tools/tests -t . -p "test_*.py"
```

### 3.3 示例场景对照与冒烟测试

使用内建的四个基础场景运行单局与批次模拟（需开启 `diagnostics` 特性），确认无 Panic 与非正常退出：

```powershell
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/duel.json 42
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/team-waves.json 42
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/revival-budget.json 42
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/anaxa-s2-full-battle.json 42

cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- batch core/battle/examples/duel.json 100 42
```

### 固定操作回放

按[固定操作回放用法](usage.md#13-固定操作回放)用种子 `42` 分别运行换波和复活样例。`cli` 回归核对每步的实际操作、事件区间与同种子 `simulate` 的完整报告，并重跑比较结果。两份固定脚本选择与这些样例的默认策略一致。

| 样例 | 核对结果 |
| --- | --- |
| `team-waves` | 49 个决策点、18 个 Action。AV 约 272.73 时第一波清空，终结技先入队，推进时扣 60 能量，命中第二波的 `wave2_boss`。两波清场获胜，剩余约 954.55 AV。 |
| `revival-budget` | 13 个决策点、4 个 Action。两次死亡后各恢复到 100 HP 并扣 25 AV，罚时没有推进实际时钟。本场推进 300 AV，耗尽预算判负，剩余 0 AV。 |

两组均为合成场景。脚本核对换波后终结技的登记能量、实际扣能和首段伤害，以及两次复活的生命和罚时。将第一次复活的预期罚时改为 26 AV，诊断在第 4 步的事件 16 返回错误，报告实际为 25 AV。用真实战斗记录时，按操作核对执行者、目标、事件顺序和可见数值，再对照终局结算。

### 那刻夏 S2 样例

[`anaxa-s2-full-battle.json`](../../../core/battle/examples/anaxa-s2-full-battle.json) 在种子 42 下返回 `victory / enemies_defeated`。场上容量为 2，最终清空 6 名敌人，执行 40 个 Action，消耗 1250 AV，剩余约 1150 AV。样例诊断 `settlement.hp_after` 为 63。

### 开拓银轨开场事件

4.7 赛季 2 配置为中立战斗事件 `62603`，伤害计入我方，初始列车在首波向全体敌人登记插入。基础列车为五段 15% 和末段 25%，模板的命中类别为 `Insert`。 [首波配置](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/Config/ConfigAbility/BattleEvent/GridFight/4.7/OriginAbility/GridFight_Origin_100502_StageAbility.json#L193-L274) · [六段效果与结束事件](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/Config/ConfigAbility/BattleEvent/GridFight/4.7/OriginAbility/BattleEvent_GridFight_Origin_100502_01_Ability.json#L1131-L1245)

运行 `cargo test -p battle-core --features diagnostics --test opening_battle_event --locked`。合成三目标样例把 18 笔命中结算为 AV 0 的一个动作，事件来源均为 `62603`。回归还覆盖开场清空首波后的请求、来源 ID 冲突、全灭判定以及普通直伤系数的对照。

样例验收事件身份、动作时序和配置接口。列车的绝对伤害仍需同场 `LevelBaseAttack`、原生 `ByBaseDamage` 乘区和有效目标面板。

### 普通虚数击破

赛季 2 所用的 4.7 公共解包配置在普通虚数击破模板中读取目标最大韧性。解码后的韧性与元素项为 `0.5 × (2 + 最大韧性 / StanceUnit) / 4`。本模块以 HSR_Nous 的击破公式为数值参考，应用击破来源的等级倍率和击破特攻，以及防御、抗性、穿透和易伤乘区。普通增伤、暴击、独立增伤、通用减伤及货币战争我方直接伤害系数在这条计算中均取中性值。 [4.7 普通虚数击破模板](https://github.com/DimbreathBot/TurnBasedGameData/blob/dad9155eb00fdeda31fb8456507c53590d49071f/Config/ConfigGlobalTaskListTemplate/GlobalTaskListTemplate.json#L1350-L1431) · [HSR_Nous 固定版本规则](https://github.com/pzc2004/HSR_Nous/blob/00185ddc4d57039894caf19467f602e716329e40/src/hsr_nous/sim_schema/rulebook.yaml#L389-L395)

从仓库根目录运行 `cargo test -p battle-core --features diagnostics --test imaginary_break --locked`。基准用 80 级来源、100% 击破特攻、1000 目标防御、120 最大韧性和 10 韧性单位，击破伤害为 `6593.218275`。回归核对单次归零、多目标、护盾吸收、监听器导致的复活、缺少最大韧性和直接伤害乘区隔离。

护盾优先吸收沿用本引擎的结算约定。游戏中普通虚数击破与护盾、通用减伤及货币战争难度系数的关系待实机确认。虚数禁锢单独核对。4.7 货币战争 1-1 开战录像出现的三次 `231` 仍需确认伤害来源、目标有效面板和削韧时序。

## 4. 实机核对

当前验证覆盖人工场景和参考公式。那刻夏样例的资料版本、近似规则和覆盖范围见场景的 `provenance` 与 `mechanics.assumptions`，技能链、队列补位和轮次同刻顺序待按实机样本核对。

以一场连续录像为单位，记录游戏版本、赛季、关卡和难度。保留实际阵容、装备、战前有效面板、初始能量和战技点、各轮 AV、敌人入场顺序及操作序列。将击中、回能、追击、补位、复活罚时和终止时点与模拟事件逐项对照。节点结算按[项目验证计划](../../project/roadmap.md#战斗与节点结算核对)采样。
