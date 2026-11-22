import { html, nothing } from "lit";
import type { Account, AccountFileKind, AccountPort } from "./contracts";
import { importReportView, inventorySummary } from "./account-import";
import type { Run, Spread } from "./folio";

export function accountManager(
  port: AccountPort,
  redraw: () => void,
  run: Run,
  select: (account: Account) => void,
  remove: (id: string) => void,
  dismiss: () => void,
  replay: (request: Record<string, unknown>, seed: string) => void,
  selected?: string,
) {
  let selectedId = selected;
  let rename = "";
  let confirming = false;
  let pending = false;
  let error = "";
  let exported = "";

  function close() {
    if (pending) return;
    dismiss();
  }

  function action(work: () => Promise<void>) {
    return run(async () => {
      pending = true;
      error = "";
      exported = "";
      redraw();
      try {
        await work();
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
      } finally {
        pending = false;
      }
    });
  }

  function download(id: string, kind: AccountFileKind) {
    return action(async () => {
      exported = await port.exportAccount(id, kind);
    });
  }

  return (accounts: Account[], warnings: string[]): Spread => {
    const local = accounts.filter(account => account.origin);
    const account = local.find(entry => entry.id === selectedId) ?? local[0];
    const origin = account?.origin;

    return {
      title: "账号管理",
      subtitle: "本地账号库",
      workspace: "account",
      headingTool: html`<button class="account-back" ?disabled=${pending} @click=${close}>← 角色档案</button>`,
      tools: html``,
      left: html`<div class="account-workspace">
        <h2>已保存账号</h2>
        ${local.length ? html`<div class="local-account-list">
          ${local.map(entry => html`<button aria-pressed=${entry === account} ?disabled=${pending}
            @click=${() => {
              selectedId = entry.id;
              rename = "";
              confirming = false;
              error = "";
              exported = "";
              redraw();
            }}>
            <span><strong>${entry.name}</strong><small>${entry.source}</small></span>
            <small>${entry.characters.length} 名角色</small>
          </button>`)}
        </div>` : html`<p class="account-lead">本地账号库为空。导入或生成账号后会显示在这里。</p>`}
        ${warnings.length ? html`<details class="account-warnings">
          <summary>${warnings.length} 个本地文件无法读取</summary>
          <ul>${warnings.map(warning => html`<li>${warning}</li>`)}</ul>
        </details>` : nothing}
      </div>`,
      right: account && origin ? html`<div class="account-workspace account-review">
        <h2>${account.name}</h2>
        ${account.summary ? inventorySummary(account.summary) : nothing}
        ${origin.kind === "imported" ? html`
          <p class="account-provenance">${origin.filename}</p>
          ${origin.report ? importReportView(origin.report) : html`<p class="account-provenance">统一账号格式 · 校验通过</p>`}
        ` : html`
          <dl class="account-generated-summary">
            <div><dt>金数</dt><dd>${origin.report.goldCount}</dd></div>
            <div><dt>平均有效词条</dt><dd>${origin.report.meanEffectiveRolls.toFixed(2)}</dd></div>
            <div><dt>随机种子</dt><dd>${origin.seed}</dd></div>
            <div><dt>核心角色</dt><dd>${origin.report.coreCharacterIds.join("、") || "无"}</dd></div>
            <div><dt>核心金数占比</dt><dd>${(origin.report.coreGoldShare * 100).toFixed(1)}%</dd></div>
            <div><dt>核心遗器得分占比</dt><dd>${(origin.report.coreRelicShare * 100).toFixed(1)}%</dd></div>
            <div><dt>搜索次数</dt><dd>${origin.report.attempts}</dd></div>
          </dl>`}
        <form class="account-rename" @submit=${(event: SubmitEvent) => {
          event.preventDefault();
          return action(async () => {
            const updated = await port.renameAccount(account.id, rename || account.name);
            select(updated);
            rename = "";
          });
        }}>
          <label class="account-field">账号名称
            <input required maxlength="100" .value=${rename || account.name} ?disabled=${pending}
              @input=${(event: Event) => {
                if (event.currentTarget instanceof HTMLInputElement) rename = event.currentTarget.value;
              }} />
          </label>
          <button ?disabled=${pending} type="submit">保存名称</button>
        </form>
        <div class="account-workspace-actions">
          <button class="primary" ?disabled=${pending} @click=${() => { close(); select(account); }}>打开角色档案</button>
          <button ?disabled=${pending} @click=${() => download(account.id, "account")}>导出账号 JSON</button>
          <button ?disabled=${pending} @click=${() => download(account.id, "report")}>导出报告</button>
          ${origin.kind === "imported" ? html`<button ?disabled=${pending} @click=${() => download(account.id, "original")}>导出原文件</button>`
            : html`<button ?disabled=${pending} @click=${() => download(account.id, "request")}>导出请求</button>
              <button ?disabled=${pending} @click=${() => { close(); replay(origin.request, origin.seed); }}>按原参数再生成</button>`}
        </div>
        ${error ? html`<p class="account-error" role="alert">${error}</p>` : nothing}
        ${exported ? html`<p class="account-provenance" role="status">已导出至 ${exported}</p>` : nothing}
        ${confirming ? html`<div class="account-delete">
          <p>删除本地账号「${account.name}」及其保存的来源数据？</p>
          <button ?disabled=${pending} @click=${() => { confirming = false; redraw(); }}>取消</button>
          <button class="primary" ?disabled=${pending} @click=${() => action(async () => {
            await port.deleteAccount(account.id);
            rename = "";
            confirming = false;
            remove(account.id);
          })}>确认删除</button>
        </div>` : html`<button class="account-remove" ?disabled=${pending}
          @click=${() => { confirming = true; redraw(); }}>删除本地账号</button>`}
      </div>` : html``,
    };
  };
}
