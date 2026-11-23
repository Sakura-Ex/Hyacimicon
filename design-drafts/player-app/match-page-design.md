# 对局页面设计

状态：设计草稿。已实现行为见[对局页](../../docs/modules/player/match.md)。面向 [对局会话接口设计](../../docs/modules/match-interface/contract.md) 与 [共享类型声明](../../apps/src/match-session-contract.ts)，在现有 Windows 玩家界面双页书册布局（见 [玩家界面说明](../../docs/modules/player/README.md)）的基础上，规划从开局到终局的对局页面结构、阶段切换、交互规则、AI 观战体验及前端模块拆分。

## 设计目标与基线

对局页面主要服务于两个场景：**亲自操作体验模拟对局**，以及**观察 AI 模型的决策思路与完整操作过程**。

新设计保留当前已实现版本（双页固定画幅 `1700 × 960`）中已验证的视觉骨架，只按新接口扩展和替换交互能力：

- **保留的既有结构**：
  - 页首左页标题区 + 右页节点进度轨与整局血量（[match-status.ts](../../apps/src/match-status.ts)）。
  - 左页顶部横向概览胶囊条（敌人预览、负面词条、投资环境、投资策略、晶矿），下方左侧为前台 4 格、后台 9 格（4 + 5 排）、备战席 9 格的等宽竖幅角色格位与独立装备栏，右侧为可滚动、可展开无边框浮层的羁绊列表及羁绊机制区。
  - 右页上段伙伴与队伍等级区、中段并排双滚动窗口（装备 / 消耗品）、下段 5 格商店竖幅报价卡、下方「本步建议」与备选区，以及底部固定操作栏。
- **新接口带来的核心变化**：
  - 全流程状态由互斥的 `MatchPhase`（11 种阶段）和 `revision` 驱动，界面不自行推断游戏规则。
  - 所有可执行交互（点击、拖拽、刷新价、禁用态）严格按当前版本的 `legalActions` 映射并提交 `actionId`。
  - 支持手动与模型控制权交接（`setController`）及实时推送（`subscribeMatch`）。
  - 支持 23 格完整路线（新增 `supply` 补给节点）、可开启晶矿、可交互羁绊机制、三类可往返备战的待选阶段，以及战斗交接三态和战后结算明细。

## 入口与会话生命周期

- **首次进入对局**：每个账号在当前应用会话内单独记录已创建的活动 `matchId`。当前账号没有活动 `matchId` 时，打开「对局」书签进入**开局配置页**，由玩家确认参数后调用 `createMatch` 创建对局。
- **恢复活动对局**：当前账号在本次会话中已有活动 `matchId` 时，切回「对局」书签调用 `getMatch` 与 `subscribeMatch` 恢复该局及实时推送。
- **重新开局**：处于 `finished`（`cleared` / `healthDepleted`）阶段时，终局页提供「开启新对局」按钮，返回开局配置页。
- **开发预览**：浏览器开发模式继续保留 `?page=match&scenario=...` 预设参数，可直接进入指定阶段用于布局与交互验收。

### 开局配置页（`createMatch`）

| 区域 | 内容与字段 |
| --- | --- |
| 左页 | 当前所选账号摘要（账号名、角色与练度概览）及开局参数说明 |
| 右页 | 表单填写 `rulesVersion`（规则版本）、`enemyDifficulty`（敌人难度）、`affixCount`（负面词条数量）及 `randomness`（自动随机 / 指定种子 `seed`） |
| 底栏 | 主按钮「开始新对局」，提交成功后保存 `matchId` 并进入 `investmentEnvironment` 阶段 |

## 页首与全局状态栏

沿用现有两段式页首，按新契约扩充状态展示：

1. **左页标题区**
   - 标题右侧使用 `Spread.badge` 展示当前阶段标签（`投资环境待选`、`备战中`、`投资策略待选`、`补给待选`、`主角光环待选`、`幕后茶歇`、`茶歇领奖`、`战斗中`、`战斗暂离`、`战斗暂不可用`、`对局结束`）。
   - 副标题行右侧放置**控制权胶囊**：
     - 手动控制：显示「手动控制」，附次级按钮「交给模型」（展开选择可用模型并调用 `setController`）。
     - 模型接管：以强调色显示「模型接管 · `<modelId>`」，附主按钮「收回控制」。
     - 订阅断线：显示「连接中断 · 重连」轻状态提示。
2. **右页进度轨与整局血量**
   - 按 `snapshot.route` 与 `snapshot.position` 渲染当前位面节点（第一位面 9 格，第二、三位面各 7 格），新增 `supply` 节点类型（`1-5`、`2-3`、`3-3`）并使用独立图标与颜色区分。
   - `position === null`（开局选投资环境）时，进度轨显示「开局准备 · 尚未进入 1-1」。
   - 处于 `investmentStrategy` 待选时，节点号显示即将进入的目标节点（如 `2-2`），旁注「投资策略待选 · 尚未入节点」，明确此时展示的是上一场战斗结算后的金币与晶矿，入节点收益在确认策略后发放。
   - 进度节点悬停提示包含 `position.enteredTriggerNodes`（已正式进入的常规节点计数），便于核对按节点触发的效果。

## 左页设计

### 1. 开局前情报页（`preparation === null`）

在 `investmentEnvironment` 阶段尚未生成阵容与商店，左页改为展示当前已公开的本局情报：

- **竞争对手（`rival`）**：名称与说明。
- **三位面对手预览（`opponents`）**：按位面 1、2、3 分组列出已知敌人与首领，标注具体节点对应关系尚未揭示。
- **负面词条（`affixes`）**：逐条展示名称与具体效果。

### 2. 顶部概览条（`preparation !== null`）

保留现有 5 枚胶囊单行排布，浮层按新接口补齐详情与交互：

| 胶囊 | 摘要内容 | 展开面板内容与交互 |
| --- | --- | --- |
| 敌人预览 | `rival.name` 及已揭示敌人数 | 上段展示当前节点 `revealedEnemies`（未揭示时提示待揭示）；下段按位面 1/2/3 分组列出 `opponents` 的普通敌人与首领。 |
| 负面词条 | `affixes.length` | 展示每条词条的名称与说明。 |
| 投资环境 | `environment.name` | 展示已选投资环境的名称与说明。 |
| 投资策略 | `selectedInvestments.length` | 列出已选策略；若 `selectedAuras` 非空，在下方附「历次主角光环」记录，区分当前生效与历史难度提升。 |
| 晶矿（右对齐） | 按白/蓝/金/彩品质色点汇总数量 | 列出 `preparation.crystals` 各品质堆叠；在 `legalActions` 存在对应 `openCrystal` 时提供「开启」按钮。备战席满 9 格时显示「备战席已满，暂不可开启晶矿」。 |

### 3. 阵容区交互

保留前台 4 格、后台 9 格、备战席 9 格的固定格位几何和竖幅卡样式，新增以下能力：

- **角色拖拽换位（`move`）**：在 `legalActions` 提供 `move` 的阶段（`preparation`、`spotlight` 备战视图、`battleAway`），可直接将角色拖至合法空位移动，或拖至另一名角色格位互换。拖拽期间仅高亮合法落点；点击角色仍可在右页详情中使用按钮完成 `move` 与 `sell`，保证键盘与精确操作可用。
- **装备与消耗品拖拽（`equip` / `combine` / `useItem`）**：继续支持从右页拖至左页角色或已穿戴装备。目标命中多条合法动作时弹出动作选择；对已穿戴简易装备的角色再拖入简易装备时，落点预览标示「合成」。
- **主角光环左右联动**：处于 `spotlight` 选择视图时，左页高亮 `phase.eligiblePieceIds` 中的可选角色，点击角色直接选中并同步右页目标角色选择。
- **光环徽记**：拥有 `selectedAuras` 中 `effectActive: true` 的角色在头像角标显示光环标记，角色详情页完整列出当前生效光环与历史难度提升。

### 4. 羁绊栏与羁绊机制

- **羁绊列表**：沿用现有紧凑列表与无边框展开浮层，展示 `TraitProgress` 的图标、名称、当前人数、下一档人数与完整激活档位。
- **羁绊机制区（`preparation.mechanisms`）**：位于羁绊列表下方，升级为可交互机制卡（覆盖 `aha`、`wishPower`、`trailblazeShop`、`chimeraGarden`）：
  - 展示机制名称与 `counters` 计数。
  - **`once`（一次性选择）**：未选择且存在 `chooseMechanism` 动作时显示入口，点击在右页打开选择面板；确认后锁定并展示已选结果。
  - **`changeable`（可反复更改）**：显示当前生效选项 `selectedOptionId` 与「修改」按钮；点击触发 `openTraitChoice`。当 `phase.openTraitChoice` 非空时，右页展示该羁绊的选择视图，逐项标出「当前生效」（`selectedOptionId`）与「待确认」（`draftOptionId`）：点选候选发送 `selectTraitOption`（不改变当前生效项），点击「确认」发送 `confirmTraitChoice` 应用并关闭（未改选也可直接确认保留原项）。
  - 带 `source: { kind: "trait", traitId, mechanism? }` 的其他合法动作挂在对应机制卡下直接触发。
  - 在投资策略、补给、主角光环待选阶段（含返回备战视图）以及战斗相关阶段，羁绊机制区呈只读态，不开放操作。

## 右页设计：阶段驱动视图

右页严格按 `MatchPhase.kind` 渲染主区，并在常规备战与待选阶段保留一致的顶部提示区与底部固定操作栏。

### 1. 全阶段映射表

| 阶段（`phase.kind`） | 左页状态 | 右页主区 | 主要动作 |
| --- | --- | --- | --- |
| `investmentEnvironment` | 开局情报页（`rival` / `opponents` / `affixes`） | 3 张环境候选卡，每张独立一次「刷新」按钮 | `refreshChoice`、`chooseInvestmentEnvironment` |
| `preparation` | 完整概览 + 可操作阵容 + 可操作羁绊机制 | 战后结算摘要 + 伙伴/等级区 + 装备/消耗品双窗 + 5 格商店 + 本步建议与备选 | 常规运营动作、`openSpotlight`、羁绊动作、`startBattle` |
| `investmentStrategy`（`view: "selection"`） | 完整概览 + 只读阵容与羁绊（可见刚结算的金币与晶矿） | 上场结算摘要 + 3 张策略卡（带银/金/彩品质标识与单格一次刷新） | `refreshChoice`、`chooseInvestmentStrategy`、`returnToPreparation` |
| `investmentStrategy`（`view: "preparation"`） | 仅晶矿浮层可开启，其余只读 | 顶部横幅提示「投资策略待选 · 仅可开启晶矿」+ 受限备战视图 | `openCrystal`、`returnToSelection` |
| `supply`（`view: "selection"`） | 完整概览 + 只读阵容与羁绊 | 5 组补给卡（竖幅角色 + 附带装备/宝石图标与名称），顶部带整组刷新一次 | `refreshSupply`、`takeSupply`、`returnToPreparation` |
| `supply`（`view: "preparation"`） | 仅晶矿浮层可开启，其余只读 | 顶部横幅提示「补给待选 · 仅可开启晶矿」+ 受限备战视图 | `openCrystal`、`returnToSelection` |
| `spotlight`（`view: "selection"`） | 高亮 `eligiblePieceIds`，点击同步选中目标角色 | 2 张光环候选卡（带单格刷新、已刷新次数 / 优惠上限 / 本次价格）+ 目标角色联动选择器 | `refreshChoice`、`chooseSpotlight`、`returnToPreparation` |
| `spotlight`（`view: "preparation"`） | 允许换位（`move`）与开启晶矿（`openCrystal`），其余只读 | 顶部横幅提示「主角光环待选 · 可调整站位与开启晶矿」+ 受限备战视图 | `move`、`openCrystal`、`returnToSelection` |
| `teaBreak` | 完整概览 + 只读阵容与羁绊 | 展示 `companion` 与茶歇选项列表 | `chooseTeaBreak` |
| `teaBreakReward` | 完整概览 + 只读阵容与羁绊 | 展示已选茶歇结果及待领说明（含原奖励关晶矿与常规金币，经验 +0） | `claimTeaBreakReward` |
| `battle` | 完整概览 + 锁定阵容 | 战斗进行面板（节点、当前 `attemptId`、敌方信息、前往「战斗」书签入口） | `restartBattle`（沿用同随机序列）、`leaveBattle` |
| `battleAway` | 完整概览 + **允许拖拽/点击换位（`move`）** | 顶部提示「战斗暂离：仅可调整站位，重进将使用新随机序列」+ 阵型确认 | `move`、`reenterBattle` |
| `battleUnavailable` | 完整概览 + 只读阵容 | 错误卡展示 `phase.message`，注明未扣血、未发奖励、未推进节点 | `restartBattle`、`leaveBattle` |
| `finished` | 终局阵容、羁绊与完整概览 | 通关（`cleared`）或血量耗尽（`healthDepleted`）结算总览、历次投资与光环回顾 | 开启新对局 |

### 2. 常规备战工作台细节

- **战后结算条（`latestSettlement`）**：在 `latestSettlement` 非空时，右页顶部提供可折叠/可关闭的单行摘要，显示血量变化（`healthBefore → healthAfter`、失血 `healthLost`、固定回复 `+fixedRecovery` 及 `otherHealthDelta`）、金币明细（`baseGold` + `interestGold` + `streakGold`）、结算后连胜 `hotStreakAfter`、经验 `+experienceGained` 与新获晶矿 `crystalsGranted`。
- **伙伴、光环与等级区**：保留 `上阵人数`、`Lv.X · experience / experienceToNextLevel` 与 `购买经验` 按钮（直接显示 `legalActions` 的 `goldCost`）。首领备战且 `phase.spotlightAvailable` 为真时，同行醒目展示「选择主角光环 ›」（`openSpotlight`）。
- **装备与消耗品**：接口的 `preparation.inventory` 为统一 `ItemStack[]`，前端按公共图鉴的 `definitionId` 分为「装备」与「消耗品」两栏并排滚动显示，保留既有两行滚动窗口和拖拽预览。
- **商店区**：
  - 标题行保留「金币 / 火热连胜 / 预计利息」折叠按钮，浮层补充 `interestCap`（利息上限）。
  - 刷新按钮在 `shop.freeRefreshes > 0` 时显示「免费刷新 · N」，否则读取 `refreshShop` 的 `goldCost`。
  - `shop.slots` 固定保留 5 格占位；已购买格位（`null`）显示虚线空位，避免其余卡片横移。
- **AI 建议与实战回填**：继续保留商店下方的「本步建议」卡片（含推荐理由、预期变化指标与备选方案切换）和底部「记录实际操作」入口，便于在手动复盘与 AI 对照时随时查看建议和记录实测值。
- **底部固定操作栏**：左侧放「重读局面」与「记录实际操作」（次级样式）；右侧放「查看本局战斗」与当前阶段主操作按钮（常规备战为「开始战斗」）。

## AI 观战、控制权交接与同步机制

1. **模型接管态（`controller.kind === "model"`）**
   - 页首控制权胶囊切换为模型接管样式，随时可点击「收回控制」向服务提交 `setController({ controller: { kind: "manual" } })`。
   - 右页上方显示紧凑的**AI 操作流水条**，实时消费 `subscribeMatch` 推送的 `actionApplied`（展示动作摘要与 `revision`），可展开回看本局最近操作序列，结合下方「本步建议」完整呈现 AI 的决策过程。
   - 页面自动进入只读观战模式：禁用所有手动写入按钮与拖拽，仅保留只读信息展开（概览、羁绊、角色装备详情、建议对比）和「收回控制」。
2. **版本控制与冲突恢复**
   - 订阅流按 `revision` 严格去重，忽略版本号小于等于本地的旧推送。
   - 同一时刻只允许一条在途写入，收到新快照后整体替换局面并清理过期的详情选中态。
   - 错误码显式提示并按需恢复：
     - `revisionConflict`：自动调用 `getMatch` 同步最新快照并提示局面已更新。
     - `controllerMismatch`：同步当前控制方并提示操作已被拒绝。
     - `actionUnavailable`：提示当前阶段不允许该操作。

## 前端模块拆分方案

建议按以下职责拆分 `apps/src/`，保持 `apps/src/match-session-contract.ts` 作为唯一契约定义：

| 模块 | 职责 |
| --- | --- |
| [match-session-contract.ts](../../apps/src/match-session-contract.ts) | 共享接口类型声明（只读契约，不夹带展示字段） |
| `apps/src/match-catalogue.ts` | 按 `rulesVersion` 解析 `unitId` / `definitionId` / `traitId` 的名称、立绘、图标、装备/消耗品分类，并将 `MatchAction` 格式化为可读操作文案 |
| [contracts.ts](../../apps/src/contracts.ts) 与 [match-port.ts](../../apps/src/match-port.ts) | `MatchPort` 继承 `MatchService`，补充 `matchInsights`（建议、模型列表、实战回填字段）与 `saveFeedback`，提供 Tauri / HTTP 适配器及 `MatchError` 文案转换 |
| `apps/src/demo-match.ts` 与 [demo.ts](../../apps/src/demo.ts) | 实现完整 11 阶段、23 格路线、AI 演示模型自动出招与 `subscribeMatch` 推送的本地演示服务，支持按 `scenario` 跳转各阶段验收 |
| [match-drag.ts](../../apps/src/match-drag.ts) | 直接基于 `LegalAction[]` 计算装备/消耗品落点（`equip` / `combine` / `useItem`）与角色换位落点（`move`） |
| [match-status.ts](../../apps/src/match-status.ts) | 23 格路线渲染（含 `supply`）、`position === null` 与待选阶段提示、整局血量显示 |
| `apps/src/match-board.ts` | 左页开局情报页、概览条（含可开启晶矿）、可拖拽阵容格位、主角光环高亮联动、羁绊栏与可交互羁绊机制 |
| `apps/src/match-phases.ts` | 开局配置页、11 种阶段专用面板、三类待选返备战横幅、战后结算条、羁绊选择面板、模型观战流水条 |
| [match.ts](../../apps/src/match.ts) 与 [match.css](../../apps/src/match.css) | 对局页顶层状态协调（订阅、控制权交接、选中栈、拖拽态、建议与实战回填）及样式 |

## 验收检查项

1. **开局与全路线**：未开局账号进入开局配置页并创建对局；选定投资环境后进入 `1-1`；进度轨完整显示 23 格路线与 `1-5` / `2-3` / `3-3` 补给节点。
2. **待选往返与操作隔离**：`investmentStrategy`、`supply`、`spotlight` 均支持「返回备战 ↔ 回到选择」，保持原候选与刷新次数；策略与补给备战视图仅允许开晶矿，光环备战视图允许开晶矿与换位，三者均关闭羁绊操作。
3. **主角光环左右联动**：在 `spotlight` 选择视图中，点击左页高亮角色可同步切换右页目标角色，且保留历史光环难度记录展示。
4. **晶矿与羁绊机制**：常规备战可开启各品质晶矿，备战席满 9 格时给出提示并禁用；`once` 选择确认后锁定，`changeable` 选择完整走通打开、切换草稿、直接确认或改选确认三步流。
5. **战斗交接与结算**：覆盖 `battle`（重开/暂离）、`battleAway`（仅可换位 + 重进）、`battleUnavailable`（错误提示 + 重试/暂离），战后正确展示 `latestSettlement` 明细，血量归零或通关进入 `finished`。
6. **AI 观战与控制权交接**：手动与模型可双向切换，模型操作经订阅流实时刷新界面与操作流水，保留「本步建议」与「记录实际操作」入口；交还控制后过期模型请求被拒绝。
7. **布局回归**：常规备战满员（前台 4 / 后台 9 / 备战席 9 / 14 羁绊 / 5 商店卡）在 `1360 × 768` 与 `1560 × 900` 下保持单行概览条、完整阵容格位几何与零遮挡。
