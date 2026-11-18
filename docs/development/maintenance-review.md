# Codex 代码结构审查

`Codex maintenance review` 在同仓库 PR 以 `main` 为目标时运行。创建、重新打开和推送新提交都会触发审查，草稿 PR 也适用。评审按照[目录与模块职责](project-layout.md)和[开发约定](development-guide.md)，核对这次改动引入的目录、依赖、接口、诊断分离和迁移问题。可直接编辑[审查规则](../../.github/codex/maintenance-review.md)。

游戏规则、算法、调度、状态变换和功能验收走[独立评审与合并流程](git-workflow.md#pr-前独立评审)。结构审查结果供作者处理和维护者查看，合并由仓库 Ruleset 管理。

## 启用

1. 将[工作流](../../.github/workflows/maintenance-review.yml)和审查规则合入 `main`。
2. 在 GitHub 仓库的 **Settings → Secrets and variables → Actions** 添加名为 `OPENAI_API_KEY` 的仓库 Secret。Codex 调用按 OpenAI API 用量计费。缺少 Secret 时工作流在 Actions 中提示配置。
3. 在同仓库打开一个面向 `main` 的 PR。打开 **Actions → Codex maintenance review** 查看运行。发现具体问题或覆盖不完整时，结论显示在 PR 的 **Conversation**。首次审查完成且发现为空时，结论显示在运行摘要。

审查使用目标分支的工作流和规则，通过 Git 对象读取 PR 内容。Codex 以只读权限运行，结论由另一个任务发布。新提交触发新审查并取消同一 PR 较早的运行。发布前再次核对目标和源提交，已有的 Codex 审查评论会更新到新结论。

## 用历史改动核对规则

| 历史改动 | 应报告的情况 | 应接受的情况 |
| --- | --- | --- |
| PR #8 的目录与源码拆分（`8647b78`、`6f87219`） | 将核心、执行入口或开发工具放入职责不匹配的模块。指出具体引用关系。 | 按职责拆分后复用公开入口。文件大小作为检查线索。 |
| PR #8 的运行时与诊断拆分（`df0b8bb`） | 默认调用因变更依赖诊断文件、指纹或详细回放。 | 由可选 `diagnostics` feature 提供开发功能，默认入口接收已组装的配置。 |
| 运营模块拆分与命名（`d5d5815`、`72cc194`） | Rust 运营规则和 HTTP 应用合在同一入口，或者资源迁移后留有会失效的读取路径。 | 应用依赖独立的 `operations-core`，Python 原型有回归对照用途，源码和安装包都有对应资源。 |

试运行时分别取每组改动前后的提交，核对评论是否指出实际职责、调用方或失效引用，以及修正后是否收敛为已解决。使用纯数值更新或游戏结算变更作为范围检查，预期此审查的发现为空。
