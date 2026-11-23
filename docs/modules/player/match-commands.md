# 对局指令契约

以 [TypeScript 类型](../../../apps/src/contracts.ts)为准，接入入口见 [MatchPort 适配器](../../../apps/src/match-port.ts)。首次读取按账号获取对局。一次操作提交一条指令，成功返回完整 `Match`。前端使用返回值刷新阵容、装备、经济、商店、节点和可用指令。

| 用途 | HTTP | Tauri |
| --- | --- | --- |
| 读取当前局面 | `GET /match?accountId=...` | `player_match({ accountId })` |
| 执行一条指令 | `POST /match/commands`，请求体为 `MatchCommandInput` | `player_execute_match_command({ input })` |
| 保存实测记录 | `POST /match/feedback`，请求体为 `Feedback` | `player_save_feedback({ input })` |

浏览器在启动或构建时配置 `VITE_MATCH_API_BASE`，例如设置为 `http://127.0.0.1:3000/v1`。跨源开发服务需允许 Vite 页面来源。Windows 桌面端在对应 Tauri 命令注册后设置 `VITE_MATCH_TRANSPORT=tauri` 并重新构建。默认演示数据可用于检验界面。

## 状态和写入

- `Match.commands` 列出当前版本能执行的具体指令。每条指令携带 `id`、可读 `label`、可选 `detail` 和结构化 `operation`。对同名角色或同名装备，标签或说明应能区分目标位置、来源或规格。
- `Piece.id` 是图鉴角色 ID，`pieceId` 区分本局中的角色实例，`slot` 是所属阵容区域从零开始的位置。`itemId` 在局内装备、库存和消耗品间唯一，可表示实例或可操作的堆叠。`offerId` 对应一条报价。
- 玩家选中目标后，前端发送 `{ "matchId": "run-1", "revision": 7, "commandId": "equip-9" }`。服务在一次写入内确认对局归属、版本、指令及其合法性，执行后递增版本。指令 ID 的有效范围是发布它的对局版本。过期、重复或不适用的指令保持局面原样，HTTP 版本冲突返回 `409`，其他业务错误可返回含 `message` 的 JSON。Tauri 可返回可读错误文本。
- 前端等待响应后才替换局面，同一时刻提交一条写入。版本冲突或网络结果不明时可通过“重读局面”核对。服务负责费用、站位、合成和装备限制、奖励结算、随机结果与节点推进。

| 入口 | 指令类型 |
| --- | --- |
| 装备及消耗品详情 | `equip`、`combine`、`useItem`，按指令提供角色、第二件装备或消耗品目标 |
| 角色详情 | `move`、`sell`，移动时带目标区域和位置 |
| 商店和推进 | `buy`、`lockShop`、`refreshShop`、`buyExperience`、`advance` |
| 主角光环 | `chooseAura`，带事件 ID、角色实例 ID 和光环选项 ID |
| 幕后茶歇 | `teaBreak`，带事件 ID 和选项 ID |

服务返回 `event` 表示当前有可打开的事件，并在 `commands` 中列出属于该事件的选择。主角光环确定后可返回 `aura` 供界面展示角色及效果。茶歇结束后返回更新后的奖励和节点状态。演示预设覆盖穿戴、拆装、购买、刷新、锁店和两类事件。其余操作可用接入端提供的指令验收。
