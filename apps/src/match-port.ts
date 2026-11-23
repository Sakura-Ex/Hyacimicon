import type { Match, MatchPort } from "./contracts";
import { invoke } from "@tauri-apps/api/core";

export function tauriMatchPort(): MatchPort {
  return {
    match: accountId => invoke("player_match", { accountId }),
    executeMatchCommand: input => invoke("player_execute_match_command", { input }),
    saveFeedback: input => invoke("player_save_feedback", { input }),
  };
}

export function httpMatchPort(baseUrl: string): MatchPort {
  const base = baseUrl.replace(/\/+$/, "");
  async function request(path: string, body?: object): Promise<Match> {
    const response = await fetch(base + path, body ? {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      redirect: "error",
    } : { cache: "no-store" });
    if (!response.ok) {
      if (response.status === 409) throw new Error("局面版本已更新，请重新读取局面。");
      const problem = await response.json().catch(() => null);
      throw new Error(typeof problem?.message === "string" ? problem.message : `对局服务请求失败（${response.status}）`);
    }
    return response.json();
  }
  return {
    match: accountId => request("/match?" + new URLSearchParams({ accountId })),
    executeMatchCommand: input => request("/match/commands", input),
    saveFeedback: input => request("/match/feedback", input),
  };
}
