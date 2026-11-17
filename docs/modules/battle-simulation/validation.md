# 目录整理与行为验证

日期：2026-11-17。参照 `origin/main` 的 `ecc0968`（账号模块）整理战斗模块。本次目录和复用调整沿用 `battle_v2` 输入及现有规则。

本页记录目录整理提交`6f87219`的验证。后续分步接口与负责人确认的结算时点见[外部选招与分步执行](stepping.md)，该页对应新增的`session.rs`回归。历史字节对照记录保留为目录整理的证据。

## 验收条件

- 输入定义、统一校验、状态推进、公式、Python配置编译和合成场景可以按目录找到。
- 现有Rust公开路径、JSON字段、输入指纹和随机种子的含义保持一致。
- 入场初始化、单位快照、效果序列执行、效果校验各有统一实现。
- 说明现有流程、各结构体与枚举的用途，以及保留相近代码的具体理由。

## 已执行的检查

根目录运行：

```powershell
cargo test --workspace --locked --offline
cargo clippy -p battle-core --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
git diff --check
```

`core/battle` 目录运行：

```powershell
python -m unittest discover -s tools/tests -t . -p "test_*.py"
```

Rust workspace回归、战斗crate严格静态检查和格式检查均通过；Python工具39项测试通过。覆盖公式、资源、波次、监听、嵌套效果、作用域、状态到期、有限队列、结算、取消、批次分母和命令行等现有用例。

另用同步 `main` 后、目录整理前构建的CLI保存四个公开场景的完整输出摘要，再与整理后的CLI比较：

| 场景 | 检查方式 |
| --- | --- |
| `duel.json`、`team-waves.json`、`revival-budget.json`、`anaxa-s2-full-battle.json` | 各使用种子 `0`、`42`、`18446744073709551615` 单局模拟；另以主种子 `42` 运行8场批次。 |

16组命令的完整JSON字节均一致，包括回放记录、单位结果、场景指纹、结算和批次统计。对照脚本与摘要保存在本地忽略的 `runs/`，用于本次重构核验。自动回归继续维护在crate的 `tests/` 和 `tools/tests/`。

## 游戏规则的验证状态

这些检查确认程序在目录调整前后的行为一致。真实规则的证据、人工设定和近似范围沿用[Anaxa S2场景记录](../../../core/battle/ANAXA_SCENARIO.md)。当前存在待核验的优先级方向、同刻Cycle顺序、随机元素分布和近似角色机制；游戏版本、快照与假设由场景输入及结果记录。

账号模块已合入workspace；账号生效配置到战斗输入的自动组装、完整角色导入、持续整局状态仍按各模块的计划推进。职责与数据流见[设计说明](design.md)。
