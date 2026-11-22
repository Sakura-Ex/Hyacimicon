import { html, nothing, render } from "lit";
import { accountView } from "./account";
import { desktopAccounts } from "./account-port";
import { matchView } from "./match";
import { battleWorkspace } from "./battle-workspace";
import { demoPort } from "./demo";
import type { Account, Match } from "./contracts";
import { icon, type Spread } from "./folio";
import { syncGameImages } from "./game-images";
import { centerBook, control, desktop } from "./windows";
import "@fontsource-variable/noto-serif-sc";
import "@fontsource-variable/noto-sans-sc";
import "./style.css";

const root = document.querySelector("main");
if (!root) throw new Error("找不到书册容器");
const container = root;
function editing(target: EventTarget | null) {
  return target instanceof Element && target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])') !== null;
}
container.addEventListener("contextmenu", (event) => {
  if (!editing(event.target)) event.preventDefault();
});
container.addEventListener("dragstart", (event) => {
  if (event.target instanceof HTMLImageElement) event.preventDefault();
});
window.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a" && !editing(event.target)) {
    event.preventDefault();
  }
});
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, (event) => {
    if (event.dataTransfer?.types.includes("Files")) event.preventDefault();
  });
}
const tabs = { account: "账号", match: "对局", battle: "战斗" };
type Page = keyof typeof tabs;
const params = new URLSearchParams(location.search);
const initialPage = params.get("page");
let page: Page = "account";
let accounts: Account[] | undefined;
let accountWarnings: string[] = [];
let account: Account | undefined;
let match: Match | undefined;
let busy = false,
  message = "";
let syncingImages = false;
let imageStatus = "";
let imageError = "";
const preview = demoPort();
const port = desktop ? { ...preview, ...desktopAccounts(preview) } : preview;
const accountsPage = accountView(port, draw, run, select, removeAccount);
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
  if (index < 0) accounts.unshift(a);
  else accounts[index] = a;
  account = a;
  match = undefined;
  battles.resetMatch();
  matchPage = createMatch();
  draw();
}
function removeAccount(id: string) {
  if (!accounts) return;
  accounts = accounts.filter((entry) => entry.id !== id);
  if (account?.id === id && accounts[0]) select(accounts[0]);
  else draw();
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
    const library = await port.accounts();
    accounts = library.accounts;
    accountWarnings = library.warnings;
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
async function prepareImages() {
  if (!desktop || syncingImages) return;
  syncingImages = true;
  imageError = "";
  imageStatus = "正在校验游戏图像…";
  draw();
  try {
    const report = await syncGameImages((missing) => {
      imageStatus = missing ? `正在准备游戏图像，剩余 ${missing} 张…` : "";
      draw();
    });
    if (report.missing) {
      imageStatus = `还有 ${report.missing} 张游戏图像待处理`;
      imageError = report.error ?? "图像准备未完成";
    } else {
      imageStatus = "";
    }
  } catch (error) {
    imageStatus = "游戏图像校验失败";
    imageError = error instanceof Error ? error.message : String(error);
  } finally {
    syncingImages = false;
    draw();
  }
}
function draw() {
  let spread: Spread = {
    title: tabs[page],
    subtitle: "",
    tools: html``,
    left: html`<p>${busy ? "正在读取…" : "暂无记录"}</p>`,
    right: html``,
  };
  if (page === "account") spread = accountsPage(account, accounts ?? [], accountWarnings);
  if (page === "match" && match) spread = matchPage(match);
  if (page === "battle") spread = battles.spread();
  document.title = "Hyacimicon · " + spread.title;
  render(
    html`<div class="book">
        <div class="titlebar" data-tauri-drag-region>
          <div class="brand" data-tauri-drag-region>${icon("hyacimicon")} Hyacimicon</div>
          ${desktop ? html`<div>${(["min", "max", "close"] as const).map((key, i) => html`<button aria-label=${["最小化", "最大化或还原", "关闭窗口"][i]} @click=${() => run(() => control(key))}>${icon(key)}</button>`)}</div>` : nothing}
        </div>
        <header>
          <div class="heading floral">
            <h1>${spread.title}</h1>
            ${spread.badge ? html`<b class="badge">${spread.badge}</b>` : nothing}<small
              >${spread.subtitle}</small
            >
          </div>
          <div class="tools">${spread.tools}</div>
        </header>
        <div class="pages" aria-busy=${busy}>
          <div class="page left"><div class="page-content scroll-area">${spread.left}</div></div>
          <div class="page right"><div class="page-content scroll-area">${spread.right}</div></div>
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
          : imageStatus
            ? html`<div role="status" class="notice" title=${imageError || nothing}>
                ${imageStatus}
                ${imageError ? html`<button @click=${prepareImages}>重试</button>
                  <button @click=${() => { imageStatus = ""; draw(); }}>关闭</button>` : nothing}
              </div>`
            : nothing
      }`,
    container,
  );
}
async function openFolio() {
  if (desktop) container.style.visibility = "hidden";
  const opening = run(() => navigate(initialPage === "battle" || initialPage === "match" ? initialPage : "account"));
  if (desktop) {
    try {
      const book = container.querySelector(".book");
      if (book) await centerBook(book);
    } catch (error) {
      console.warn("无法精确居中书册，使用窗口默认位置", error);
    } finally {
      container.style.removeProperty("visibility");
    }
  }
  await opening;
}

await Promise.all([openFolio(), prepareImages()]);
