# 项目目录与模块职责

按业务职责设置模块目录。`core/` 放研究工具和玩家应用共用的实现，`research/` 放账号生成、Python 绑定、训练和评估，`apps/` 放面向玩家的应用。模块通过公开接口调用，规则和状态由对应模块维护。数据流见[账号数据流](../modules/account-generation/data-flow.md#处理流程)和[系统设计](../project/architecture.md#架构与模块交互)。

## 模块目录

| 目录 | 状态 | 职责与依赖 |
| --- | --- | --- |
| `research/account_generation/` | 已有 | 根据生成条件和随机源产出原始账号配置。模块内部结构见[账号与遗器数据](../modules/account-generation/README.md)。 |
| `core/account/` | 规划 | 将程序生成的账号配置与玩家接入的数据转换为生效配置和可用资源。 |
| `core/battle/` | 待迁移 | `battle-core` 接收已组装的战斗输入，结算行动顺序、技能、伤害和状态，返回战斗结果与事件。CLI 调用同一战斗库，供调试、按种子回放和批量验证使用。 |
| `core/run/` | 规划 | 依据 `core/account/` 提供的配置维护整局状态，提供可见局面和合法操作。校验操作后组装战斗输入，调用 `core/battle/`，并在满足条件时结算结果。 |
| `core/decision/` | 规划 | 用可见局面和可用资源构造模型输入，通过 `core/run/` 评估候选操作，输出运营或战斗建议。 |
| `research/sim_native/` | 规划 | 用 PyO3 提供训练和评估共用的 Python 模拟接口，依赖 Rust 模拟核心。 |
| `research/training/`、`research/evaluation/` | 规划 | 通过 `sim_native` 调用模拟，采集轨迹、训练模型、回放对局和比较策略。 |
| `apps/player/` | 规划 | 接收玩家账号数据和当前局面，调用核心接口，展示建议并记录玩家反馈。 |

候选操作在整局状态副本上试算。实际对局达到正式结算条件时，由 `core/run/` 将结果写入当前整局状态一次。

战斗库当前在 [PR #8](https://github.com/Sakura-Ex/hsr-currency-wars-ai/pull/8) 的 `crates/battle-core/` 开发，合并前迁到 `core/battle/`，Cargo 包名改为 `battle-core`。

## Python 模拟绑定

`research/sim_native/` 维护 `sim-native` crate 的 `Cargo.toml` 和 Python 扩展 `sim_native` 的 `pyproject.toml`。扩展负责转换参数、结果和错误，模拟规则和状态由 Rust 核心维护。

- 战斗接入时，`sim-native` 通过 `../../core/battle` 路径依赖 `battle-core`，把战斗接口暴露为 `sim_native.battle`。
- 局面模拟接入时，`sim-native` 增加指向 `../../core/run` 的路径依赖，在同一个扩展中暴露 `sim_native.run`。局面观测、合法操作、状态推进和结果结算由 `core/run/` 提供。

各 Rust crate 加入仓库根目录的 Cargo workspace。战斗绑定接入时，根清单将 `core/battle` 设为默认成员。两个 Python 接口共用一个 Maturin wheel。构建、安装和验证步骤见[Python 模拟绑定使用说明](../modules/sim-native/README.md)。

## 配套目录

| 位置 | 职责 |
| --- | --- |
| `tests/<模块>/`、`examples/<模块>/` | Python 模块的自动测试和调用示例，目录名与模块对应。 |
| Rust crate 内的 `tests/`、`tests/fixtures/` 和 `examples/` | 对应 crate 的集成测试、测试样例和可运行示例。单元测试放在源码旁。 |
| `docs/project/`、`docs/game-data/`、`docs/development/`、`docs/modules/<模块>/` | 分别存放项目架构与规则、共用游戏数值、开发规范、模块设计与使用说明。 |
| `local-data/`、`runs/`、`checkpoints/` | 本地输入、运行记录和模型权重等产物，跨仓库资料按 [AGENTS.md](../../AGENTS.md) 的分工维护。 |

Python 项目和 Cargo workspace 各自在仓库根目录维护共用配置和锁文件。各模块的文档、示例和测试通过对应模块的公开接口组织。
