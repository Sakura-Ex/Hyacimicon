# Hyacimicon Player (`apps/player`)

面向《崩坏：星穹铁道》「货币战争」玩法的网页端视觉样板与交互式运营模拟器（React + TypeScript + Vite）。

## 功能视图

1. **动态运营模拟** (`LiveMatchView`)：严格对齐 [对局会话接口设计](../../docs/modules/match-interface/contract.md) 与 [对局接口规则](../../docs/modules/match-interface/README.md)，通过本地 `LocalSimulatedMatchService` 驱动 23 节点路线、投资环境/策略选择、补给节点、主角光环、商店买卖/升星/经验/锁店、前后台及备战席站位限制与拖拽换位、晶矿开启、简易装备合成、战斗进度结算与控制权交接（`AI 托管一步`）。
2. **选择弹窗样板** (`ModalGalleryView`)：集中展示投资环境、白银/黄金/棱彩投资策略、补给 5 选 1、主角光环与战斗结算报告等崩铁风格遮罩界面。
3. **视觉规范与组件库** (`DesignSystemView`)：色彩与官方 1–7 费阶梯令牌、按钮/面板原语、富文本高亮与全部 9 种 SVG 装备边框展示。
4. **真实资产图鉴** (`LiveDataArchiveView`)：直连外部图鉴数据仓库实时浏览角色、装备与投资策略。

## 数据仓库桥接

为避免将私有或大型二进制素材提交至本仓库，[vite.config.ts](vite.config.ts) 在开发与预览服务器中提供本地资产桥接（`/hsr-assets/*`、`/hsr-svg/*`、`/hsr-data/*`、`/hsr-meta/status`），按以下顺序定位 `hsr-currency-wars-data`：

1. 环境变量 `HSR_CURRENCY_WARS_DATA`
2. 环境变量 `HSR_DATA_DIR`
3. 默认同级目录 `../../../hsr-currency-wars-data`

## 开发与验证命令

在 `apps/player/` 目录执行：

```powershell
pnpm install
pnpm run dev          # 启动本地开发服务器 (http://127.0.0.1:5173/)
pnpm exec tsc --noEmit
pnpm run build
pnpm run verify:rules # 运行货币战争运营规则与会话契约断言
```

## 当前限制

- 当前由前端本地 `LocalSimulatedMatchService` 模拟 `MatchService` 会话状态，尚未连接 Rust `core/run` 与 `core/battle` 真实战斗引擎；战斗界面通过进度滑块输入战果以验证节点结算逻辑。
- 右侧“AI 实时策略参谋”与“AI 托管一步”目前基于当前可见快照与 `legalActions` 的规则启发式实现，尚未接入强化学习或搜索模型。
