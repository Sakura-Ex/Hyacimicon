import { html, nothing, render } from "lit";
import { accountView } from "./account";
import { matchView } from "./match";
import { battleWorkspace } from "./battle-workspace";
import { demoPort } from "./demo";
import type { Account, Match } from "./contracts";
import { icon, type Spread } from "./folio";
import { control, desktop } from "./windows";
import "@fontsource-variable/noto-serif-sc";
import "@fontsource-variable/noto-sans-sc";
import "./style.css";

const root = document.querySelector("main");
if (!root) throw new Error("找不到书册容器");
const container = root;
const tabs = { account: "账号", match: "对局", battle: "战斗" };
type Page = keyof typeof tabs;
const params = new URLSearchParams(location.search);
const initialPage = params.get("page");
let page: Page = "account";
let accounts: Account[] | undefined;
let account: Account | undefined;
let match: Match | undefined;
let busy = false,
  message = "";
const port = demoPort();
const accountsPage = accountView(port, draw, run, select);
const battles = battleWorkspace(port, draw, run, loadMatch);
const createMatch = () =>
  matchView(
    port,
    (m) => {
      match = m;
      draw();
    },
    draw,
    run,
    () =>
      run(async () => {
        await battles.enter("currencyWars");
        page = "battle";
      }),
  );
let matchPage = createMatch();
function select(a: Account) {
  accounts ??= [];
  const index = accounts.findIndex((other) => other.id === a.id);
  if (index < 0) accounts.push(a);
  else accounts[index] = a;
  account = a;
  match = undefined;
  battles.resetMatch();
  matchPage = createMatch();
  draw();
}
async function run(work: () => Promise<void>) {
  if (busy) return;
  busy = true;
  message = "";
  draw();
  try {
    await work();
  } catch (error) {
    message = error instanceof Error ? error.message : String(error);
  } finally {
    busy = false;
    draw();
  }
}
async function loadAccounts() {
  if (!accounts) {
    accounts = await port.accounts();
    account = accounts[0];
  }
}
async function loadMatch() {
  await loadAccounts();
  if (!account) throw new Error("请先选择账号。");
  if (!match) match = await port.match(account.id);
  return match;
}
async function navigate(next: Page) {
  if (next === "battle") await battles.enter();
  else if (next === "match") await loadMatch();
  else await loadAccounts();
  page = next;
}
function draw() {
  let spread: Spread = {
    title: tabs[page],
    subtitle: "",
    tools: html``,
    left: html`<p>${busy ? "正在读取…" : "暂无记录"}</p>`,
    right: html``,
  };
  if (page === "account") spread = accountsPage(account, accounts ?? []);
  if (page === "match" && match) spread = matchPage(match);
  if (page === "battle") spread = battles.spread();
  document.title = "Hyacimicon · " + spread.title;
  render(
    html`<div class="book">
        <div class="titlebar" data-tauri-drag-region>
          <div class="brand" data-tauri-drag-region>
            ${icon("hyacimicon")} Hyacimicon
          </div>
          ${desktop ? html`<div>${(["min", "max", "close"] as const).map((key, i) => html`<button aria-label=${["最小化", "最大化或还原", "关闭窗口"][i]} @click=${() => run(() => control(key))}>${icon(key)}</button>`)}</div>` : nothing}
        </div>
        <header>
          <div class=${page === "match" ? "heading" : "heading floral"}>
            <h1>${spread.title}</h1>
            ${spread.badge ? html`<b class="badge">${spread.badge}</b>` : nothing}<small
              >${spread.subtitle}</small
            >
          </div>
          <div class="tools">${spread.tools}</div>
        </header>
        <div class="pages" aria-busy=${busy}>
          <div class="page left">${spread.left}</div>
          <div class="page right">${spread.right}</div>
        </div>
      </div>
      <nav aria-label="书签">
        ${(Object.keys(tabs) as Page[]).map((t) => html`<button aria-current=${t === page ? "page" : nothing} @click=${() => run(() => navigate(t))}>${icon("tab-" + t)}${tabs[t]}</button>`)}
      </nav>
      ${
        message
          ? html`<div role="alert" class="notice">
              ${message}<button
                @click=${() => {
                  message = "";
                  draw();
                }}
              >
                关闭
              </button>
            </div>`
          : nothing
      }`,
    container,
  );
}
await run(() =>
  navigate(
    initialPage === "battle" || initialPage === "match"
      ? initialPage
      : "account",
  ),
);
