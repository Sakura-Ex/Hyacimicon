# 代码结构审查

`Maintenance review` 在同仓库 PR 以 `main` 为目标时运行。创建、重新打开和推送新提交都会触发审查，草稿 PR 也适用。审查通过 OpenAI Responses API 调用 GPT‑6.2 Sol，推理力度为 `medium`。评审按照[目录与模块职责](project-layout.md)和[开发约定](development-guide.md)，核对这次改动引入的目录、依赖、接口、诊断分离和迁移问题。可直接编辑[审查规则](../../.github/codex/maintenance-review.md)。

游戏规则、算法、调度、状态变换和功能验收走[独立评审与合并流程](git-workflow.md#pr-前独立评审)。结构审查结果供作者处理和维护者查看，合并由仓库 Ruleset 管理。

## 启用

1. 将[工作流](../../.github/workflows/maintenance-review.yml)和审查规则合入 `main`。
2. 在 GitHub 仓库的 **Settings → Environments** 创建或打开 `codex-review`。在 **Deployment branches and tags** 选择 **Selected branches and tags**，添加一条类型为 **Branch**、名称为 `main` 的规则。完成分支限制后，在该环境的 **Environment secrets** 添加 `OPENAI_API_KEY`。使用已领取 ChatGPT Pro API 赠送额度的 API 组织所创建的 key，可以用赠送额度支付审查调用。缺少 Secret 时工作流会提示配置。 [OpenAI API 赠送额度说明](https://developers.openai.com/api/docs/guides/chatgpt-api-credits#what-the-credits-cover)
3. 在 **Settings → Actions → Policies** 核对适用于该工作流的事件策略，确保允许 `pull_request_target`。GitHub 将从 2026 年 12 月 2 日起对受影响的公开仓库执行默认拦截策略。配置策略时将授权范围对准该工作流。 [GitHub 事件策略](https://docs.github.com/en/actions/reference/security/securely-using-pull_request_target#default-policy-for-pull_request_target)
4. 在同仓库打开一个面向 `main` 的 PR，或者为已有 PR 推送新提交。打开 **Actions → Maintenance review** 查看运行。发现具体问题或覆盖不完整时，结论显示在 PR 的 **Conversation**。首次审查完成且发现为空时，结论显示在运行摘要。

`pull_request_target` 从 `main` 读取工作流和审查脚本。首次加入这份工作流的 PR 合入后，后续 PR 事件才会触发审查。审查任务引用 `codex-review` 环境，将仓库约定、PR 差异和按需读取的文件发送给 Responses API。模型可通过预设的只读 Git 查询核对调用方。查询有次数和大小限制。差异超出预读范围时，报告标为部分完成。

发布任务使用自己的 GitHub 权限更新评论。相同提交范围的重复运行可以互相替代。发布任务按 PR 排队，核对最新的源提交和目标分支后更新已有审查评论，并标明实际使用的目标快照。同一源提交有多次审查时，评论采用沿目标分支历史推进的快照。

## 用历史改动核对规则

| 历史改动 | 应报告的情况 | 应接受的情况 |
| --- | --- | --- |
| PR #8 的目录与源码拆分（`8647b78`、`6f87219`） | 将核心、执行入口或开发工具放入职责不匹配的模块。指出具体引用关系。 | 按职责拆分后复用公开入口。文件大小作为检查线索。 |
| PR #8 的运行时与诊断拆分（`df0b8bb`） | 默认调用因变更依赖诊断文件、指纹或详细回放。 | 由可选 `diagnostics` feature 提供开发功能，默认入口接收已组装的配置。 |
| 运营模块拆分与命名（`d5d5815`、`72cc194`） | Rust 运营规则和 HTTP 应用合在同一入口，或者资源迁移后留有会失效的读取路径。 | 应用依赖独立的 `operations-core`，Python 原型有回归对照用途，源码和安装包都有对应资源。 |

试运行时分别取每组改动前后的提交，核对评论是否指出实际职责、调用方或失效引用，以及修正后是否收敛为已解决。使用纯数值更新或游戏结算变更作为范围检查，预期此审查的发现为空。
