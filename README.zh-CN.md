# HSR Currency Wars AI

[English](README.md) | 简体中文

面向 Windows 玩家的开源 AI 策略研究与辅助项目，用于《崩坏：星穹铁道》的货币战争玩法。

## 项目目标

- 探索并评估阵容、战斗决策和运营策略。
- 根据玩家可用的角色与装备提供适合该账号的策略建议。

每个已选择的战斗只模拟一次，按这次实际胜败平结果结算资源并产生下一观测，同时记录剩余行动值和终止原因。失败经历也用于训练；同场重复采样留给候选比较和离线评估。

当前框架 PR 将征求团队明确反馈，收到认可后再继续实现，具体见[框架评审问题](docs/modules/run-simulation/README.md#框架评审与继续条件)。

## 当前状态

| 模块 | 状态 |
| --- | --- |
| [账号生成器](docs/modules/account-generation/account-generation.md) | 已实现。使用随包提供的 4.7正式版游戏数据，可离线生成包含持有角色、光锥和遗器的样例账号。 |
| 账号适配器与配置转换 | 设计中。涵盖玩家账号导入、试用与保底映射、生效属性计算。 |
| [整局环境](docs/modules/run-simulation/README.md) | 框架候选已实现。每场采样一次，保存结果，按显式规则结算并记录下一观测；包含合成示例，真实节点规则和商店待接入。 |
| 运营决策器 | 设计中。当前先评审单次采样的整局框架，再接合法运营操作和训练轨迹。模型训练待实现。 |
| [战斗引擎](docs/modules/battle-simulation/README.md) | 已实现 Rust `basic_v1` 显式输入内核与回放 CLI；同场批次统计作为离线评估工具，执行已解析的面板和动作并支持确定性重放；尚不支持完整 4.7 角色机制、自动账号装配和整局扣血结算。 |
| 战斗规划器 | 计划中。比较站位、战斗操作和重试方案。 |
| 阵容研究器 | 计划中。搜索阵容和过渡路线。 |
| 实验评估器 | 计划中。回放对局并比较策略结果。 |
| Windows 玩家应用 | 已确定 Tauri + TypeScript，待接入 Rust 核心并记录玩家实际反馈。 |

项目计划、设计和开发指南见[文档入口](docs/README.md)，这些资料使用中文。

## 参考与致谢

| 仓库 | 用途 |
| --- | --- |
| [StarRailRes](https://github.com/Mar-7th/StarRailRes) | 游戏基础数据参考 |
| [TurnBasedGameData](https://github.com/DimbreathBot/TurnBasedGameData) | 游戏基础数据参考 |
| [HSR_Nous](https://github.com/pzc2004/HSR_Nous) | 战斗公式与合成验证向量（[MIT](core/battle/src/math.rs)） |

## 许可证

本项目采用 GNU 通用公共许可证第 3 版（`GPL-3.0-only`）。完整许可证文本见 [LICENSE](LICENSE)。

本项目由社区独立开发，与 HoYoverse 无隶属关系，也未获得其官方认可。游戏名称和商标属于各自权利人。
