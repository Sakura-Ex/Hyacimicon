import { isTauri } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const desktop = isTauri();
export async function openBattle(matchId: string) {
  const url = "?battle=" + encodeURIComponent(matchId);
  if (!desktop) {
    window.open(url, "hyacimicon-battle");
    return;
  }
  const existing = await WebviewWindow.getByLabel("battle");
  if (existing) {
    await emitTo("battle", "folio-battle", matchId);
    await existing.setFocus();
    return;
  }
  const view = new WebviewWindow("battle", {
    url: "index.html" + url,
    title: "Hyacimicon · 战斗详情",
    width: 1360,
    height: 768,
    minWidth: 840,
    minHeight: 540,
    decorations: false,
    transparent: true,
    shadow: false,
    backgroundColor: "#00000000",
  });
  await new Promise<void>((resolve, reject) => {
    Promise.all([
      view.once("tauri://created", () => resolve()),
      view.once("tauri://error", (e) => reject(new Error(String(e.payload)))),
    ]).catch(reject);
  });
}
export async function watchNavigation(
  main: (page: "account" | "match") => void,
  battle: (id: string) => void,
) {
  if (!desktop) return;
  await listen<unknown>("folio-page", (e) => {
    if (e.payload === "account" || e.payload === "match") main(e.payload);
  });
  await listen<unknown>("folio-battle", (e) => {
    if (typeof e.payload === "string") battle(e.payload);
  });
}
export async function goMain(page: "account" | "match") {
  if (!desktop) {
    location.href = "?page=" + page;
    return;
  }
  const main = await WebviewWindow.getByLabel("main");
  if (!main) throw new Error("主窗口已关闭。");
  await emitTo("main", "folio-page", page);
  await main.setFocus();
}
export async function control(action: "min" | "max" | "close") {
  const win = getCurrentWindow();
  if (action === "min") await win.minimize();
  else if (action === "max") await win.toggleMaximize();
  else await win.close();
}
