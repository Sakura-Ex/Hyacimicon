import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { desktop } from "./windows";

let available = new Set<string>();

export function imageUrl(source?: string): string | undefined {
  if (!desktop || !source) return source;
  const key = source.slice(1);
  return available.has(key) ? convertFileSrc(key, "game-images").replace(/%2F/g, "/") : undefined;
}

interface SyncReport {
  available: string[];
  missing: number;
  error: string | null;
}

function syncReport(value: unknown): value is SyncReport {
  return typeof value === "object" && value !== null &&
    "available" in value && Array.isArray(value.available) &&
    value.available.every((path) => typeof path === "string") &&
    "missing" in value && typeof value.missing === "number" &&
    Number.isInteger(value.missing) && value.missing >= 0 &&
    "error" in value && (value.error === null || typeof value.error === "string");
}

export async function syncGameImages(showProgress: (missing: number) => void) {
  let receiving = true;
  const onCached = new Channel<unknown>((progress) => {
    if (receiving && syncReport(progress)) {
      available = new Set(progress.available);
      showProgress(progress.missing);
    }
  });
  try {
    const result: unknown = await invoke("player_sync_game_images", { onCached });
    if (!syncReport(result)) throw new Error("无法读取游戏图像校验结果。");
    available = new Set(result.available);
    return { missing: result.missing, error: result.error };
  } finally {
    receiving = false;
  }
}
