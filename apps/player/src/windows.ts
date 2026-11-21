import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const desktop = isTauri();
export async function control(action: "min" | "max" | "close") {
  const win = getCurrentWindow();
  if (action === "min") await win.minimize();
  else if (action === "max") await win.toggleMaximize();
  else await win.close();
}
