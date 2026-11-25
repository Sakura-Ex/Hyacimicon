import { html, nothing, render } from "lit";
import { accountView } from "./account.ts";
import { desktopAccounts } from "./account-port.ts";
import { observedView } from "./observed-view.ts";
import type { Account, AccountPort } from "./contracts.ts";
import { icon } from "./folio.ts";
import { centerBook, control, desktop } from "./windows.ts";
import "@fontsource-variable/noto-serif-sc";
import "@fontsource-variable/noto-sans-sc";
import "./style.css";
import "./observed.css";

const root = document.querySelector("main");
if (!root) throw new Error("找不到书册容器");
const container = root;
const desktopOnly = async (): Promise<never> => { throw new Error("账号文件管理请使用 Windows 桌面版。"); };
const browserPort: AccountPort = {
  accounts: async () => ({ accounts: [], warnings: [] }),
  inspectAccount: desktopOnly, importAccount: desktopOnly, generateAccount: desktopOnly,
  accountCatalogue: desktopOnly, renameAccount: desktopOnly, deleteAccount: desktopOnly, exportAccount: desktopOnly,
};
const port = desktop ? desktopAccounts() : browserPort;
let accounts: Account[] = [];
let warnings: string[] = [];
let account: Account | undefined;
let page: "account" | "match" = new URLSearchParams(location.search).get("page") === "match" ? "match" : "account";
let busy = false;
let message = "";
const match = observedView(draw, localStorage);
const accountPage = accountView(port, draw, run, select, remove, false);
function select(entry: Account) {
  accounts = [entry, ...accounts.filter(previous => previous.id !== entry.id)];
  account = entry;
  draw();
}
function remove(id: string) {
  accounts = accounts.filter(entry => entry.id !== id);
  if (account?.id === id) account = accounts[0];
  draw();
}
async function run(work: () => Promise<void>) {
  if (busy) return;
  busy = true;
  message = "";
  draw();
  try { await work(); }
  catch (error) { message = error instanceof Error ? error.message : String(error); }
  finally { busy = false; draw(); }
}
function draw() {
  const spread = page === "account" ? accountPage(account, accounts, warnings) : match(account);
  document.title = `Hyacimicon · ${spread.title}`;
  render(html`<div class="book">
    <div class="titlebar" data-tauri-drag-region><div class="brand" data-tauri-drag-region>${icon("hyacimicon")} Hyacimicon · 玩家版</div>
      <div class="titlebar-actions">${desktop ? html`<button aria-label="最小化" @click=${() => run(() => control("min"))}>${icon("min")}</button>
      <button aria-label="关闭窗口" @click=${() => run(() => control("close"))}>${icon("close")}</button>` : nothing}</div></div>
    <header><div class="heading floral"><h1>${spread.title}</h1>${spread.badge ? html`<b class="badge">${spread.badge}</b>` : nothing}<small>${spread.subtitle}</small>${spread.subtitleTools ?? nothing}</div><div class="tools">${spread.tools}</div></header>
    <div class="pages" aria-busy=${busy}><div class="page left"><div class="page-content scroll-area">${spread.left}</div></div><div class="page right"><div class="page-content scroll-area">${spread.right}</div></div></div>
  </div><nav aria-label="书签">${(["account", "match"] as const).map(tab => html`<button aria-current=${tab === page ? "page" : nothing} ?disabled=${busy} @click=${() => { page = tab; draw(); }}>${icon("tab-" + tab)}${tab === "account" ? "账号" : "对局"}</button>`)}</nav>
  ${message ? html`<div role="alert" class="notice">${message}<button @click=${() => { message = ""; draw(); }}>关闭</button></div>` : nothing}`, container);
}
await run(async () => {
  const library = await port.accounts();
  accounts = library.accounts;
  warnings = library.warnings;
  account = accounts[0];
});
if (desktop) {
  try {
    const book = container.querySelector(".book");
    if (book) await centerBook(book);
  } catch (error) {
    console.warn("无法精确居中书册，使用窗口默认位置", error);
  }
}
