# 对局服务接入

研究版默认使用[演示服务](../../../apps/src/demo-match.ts)。前端已提供 [HTTP 与 Tauri 适配器](../../../apps/src/match-port.ts)。正式对局需要注册下表入口，并接入 Rust 会话与战斗服务。

## 会话与显示

[共享会话契约](../match-interface/contract.md)约定阶段、合法动作、版本、控制权和战斗交接。[类型声明](../../../apps/src/match-session-contract.ts)定义共享字段。

[会话控制器](../../../apps/src/match-session.ts)按账号恢复 `matchId`。界面从 `legalActions` 取动作 ID，写入携带 `matchId` 和 `revision`。推送到达后重读快照，并按版本去重。操作流水独立按通知版本去重，因此可显示快照已经由写入响应送达时对应的操作。

图鉴、建议、模型列表与回填字段由 `matchInsights` 返回。图鉴按规则版本中的角色、装备、羁绊定义 ID 查找，缺少图鉴资料的物品放在「待识别物品」。回填提交当前对局版本、实际操作和字段观测值。战斗书签的正式回放由战斗服务接入。

## 传输

浏览器启动或构建前设置 `VITE_MATCH_API_BASE`。跨源开发服务需允许 Vite 页面来源。桌面端注册命令后设置 `VITE_MATCH_TRANSPORT=tauri` 并重新构建。

| 操作 | HTTP | Tauri |
| --- | --- | --- |
| 开局 | `POST /match/create` | `player_create_match` |
| 读取 | `POST /match/get` | `player_get_match` |
| 执行动作 | `POST /match/actions` | `player_execute_action` |
| 交接控制 | `POST /match/controller` | `player_set_controller` |
| 订阅 | `GET /match/updates?matchId=…` | `player_subscribe_match` |
| 解除订阅 | 取消 HTTP 请求 | `player_unsubscribe_match` |
| 读取显示数据 | `POST /match/insights` | `player_match_insights` |
| 保存回填 | `POST /match/feedback` | `player_save_session_feedback` |

HTTP POST 请求体直接使用 TypeScript 输入类型，成功与业务错误均返回 `MatchResult` JSON。成功订阅使用 `application/x-ndjson`，每行一个 `MatchUpdate`，首条为快照。Tauri 命令使用 `{ input }`，订阅额外传入 `onUpdate` 通道，返回 `MatchResult<string>` 作为订阅 ID。通道以 `null` 表示结束，解除订阅传入 `{ subscriptionId }`。回填成功返回 `MatchResult<null>`。
