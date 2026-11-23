import type { Catalogue } from "./match-catalogue.ts";
import { Channel, invoke } from "@tauri-apps/api/core";
import { matchStream } from "./match-stream.ts";
import type { ActionOption, Advice, Feedback, FeedbackField } from "./contracts.ts";
import type { ChoiceOption, MatchError, MatchRef, MatchResult, MatchService, MatchUpdate, MatchVersion } from "./match-session-contract.ts";

export interface MatchInsights {
  catalogue?: Catalogue;
  models: { id: string; name: string }[];
  advice: Advice[];
  actions: ActionOption[];
  feedbackFields: FeedbackField[];
  selectedTea?: ChoiceOption;
  lastAction?: string;
}

export interface SessionPort extends MatchService {
  subscribeMatch(input: MatchRef, signal?: AbortSignal): Promise<MatchResult<AsyncIterable<MatchUpdate>>>;
  matchInsights(input: MatchVersion): Promise<MatchResult<MatchInsights>>;
  saveSessionFeedback(input: Feedback): Promise<MatchResult<null>>;
}

export function matchError(error: MatchError): string {
  switch (error.code) {
    case "revisionConflict": return "局面版本已更新，已同步当前局面。";
    case "controllerMismatch": return "当前控制方已变化，已同步控制权。";
    case "actionUnavailable": return "当前阶段已无法执行这项操作。";
    default: return error.message;
  }
}

export function tauriSessionPort(): SessionPort {
  return {
    createMatch: input => invoke("player_create_match", { input }),
    getMatch: input => invoke("player_get_match", { input }),
    executeAction: input => invoke("player_execute_action", { input }),
    setController: input => invoke("player_set_controller", { input }),
    matchInsights: input => invoke("player_match_insights", { input }),
    saveSessionFeedback: input => invoke("player_save_session_feedback", { input }),
    async subscribeMatch(input, signal) {
      const stream = matchStream<MatchUpdate>(signal);
      const onUpdate = new Channel<MatchUpdate | null>(event => {
        if (event === null) stream.finish();
        else stream.push(event);
      });
      try {
        const result = await invoke<MatchResult<string>>("player_subscribe_match", { input, onUpdate });
        if (!result.ok) {
          stream.close();
          return result;
        }
        return { ok: true, value: (async function* () {
          try {
            yield* stream.updates();
          } finally {
            await invoke("player_unsubscribe_match", { subscriptionId: result.value });
          }
        })() };
      } catch (error) {
        stream.close();
        throw error;
      }
    },
  };
}

async function* readUpdates(body: ReadableStream<Uint8Array>) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let pending = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      const lines = (pending + decoder.decode(value, { stream: true })).split("\n");
      pending = lines.pop() ?? "";
      for (const line of lines) if (line.trim()) yield parseUpdate(line);
    }
    pending += decoder.decode();
    if (pending.trim()) yield parseUpdate(pending);
  } finally {
    try { await reader.cancel(); }
    finally { reader.releaseLock(); }
  }
}

function parseUpdate(line: string): MatchUpdate {
  // The transport carries MatchUpdate from the same versioned service contract.
  return JSON.parse(line);
}

export function httpSessionPort(baseUrl: string): SessionPort {
  const base = baseUrl.replace(/\/+$/, "");
  async function request<T>(path: string, input: object): Promise<MatchResult<T>> {
    const response = await fetch(base + path, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input), redirect: "error",
    });
    if (response.headers.get("Content-Type")?.includes("application/json")) return response.json();
    throw new Error(`无法读取对局服务响应（${response.status}）。`);
  }
  return {
    createMatch: input => request("/match/create", input),
    getMatch: input => request("/match/get", input),
    executeAction: input => request("/match/actions", input),
    setController: input => request("/match/controller", input),
    matchInsights: input => request("/match/insights", input),
    saveSessionFeedback: input => request("/match/feedback", input),
    async subscribeMatch(input, signal) {
      const response = await fetch(base + "/match/updates?" + new URLSearchParams({ matchId: input.matchId }),
        { signal, cache: "no-store", redirect: "error" });
      const contentType = response.headers.get("Content-Type");
      if (!response.ok && contentType?.includes("application/json")) return response.json();
      if (!response.ok || !contentType?.includes("application/x-ndjson") || !response.body) {
        throw new Error(`无法订阅对局（${response.status}）。`);
      }
      return { ok: true, value: readUpdates(response.body) };
    },
  };
}
