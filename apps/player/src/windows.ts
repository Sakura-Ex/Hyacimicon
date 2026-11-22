import { isTauri } from "@tauri-apps/api/core";
import { currentMonitor, getCurrentWindow, PhysicalPosition, primaryMonitor } from "@tauri-apps/api/window";

export const desktop = isTauri();

export async function centerBook(book: Element) {
  const monitor = (await currentMonitor()) ?? (await primaryMonitor());
  if (!monitor) return;

  const win = getCurrentWindow();
  const [outer, inner] = await Promise.all([win.outerPosition(), win.innerPosition()]);
  const { left, top, width, height } = book.getBoundingClientRect();
  const scale = window.devicePixelRatio;
  const bookX = inner.x + (left + width / 2) * scale;
  const bookY = inner.y + (top + height / 2) * scale;
  await win.setPosition(new PhysicalPosition(
    Math.round(outer.x + monitor.position.x + monitor.size.width / 2 - bookX),
    Math.round(outer.y + monitor.position.y + monitor.size.height / 2 - bookY),
  ));
}

export async function control(action: "min" | "max" | "close") {
  const win = getCurrentWindow();
  if (action === "min") await win.minimize();
  else if (action === "max") await win.toggleMaximize();
  else await win.close();
}
