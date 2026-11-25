import type { Controller, CreateMatchInput, LegalAction, MatchAction, MatchResult, MatchSnapshot } from "./match-session-contract.ts";
import { matchError, type MatchInsights, type SessionPort } from "./match-port.ts";

type HistoryEntry = { revision: number; action: MatchAction; modelId: string };

export function matchSession(port: SessionPort, redraw: () => void, preview?: (accountId: string) => Promise<MatchResult<MatchSnapshot>>) {
  const active = new Map<string, string>();
  const histories = new Map<string, HistoryEntry[]>();
  const previewed = new Set<string>();
  let accountId: string | undefined;
  let snapshot: MatchSnapshot | undefined;
  let insights: MatchInsights | undefined;
  let subscription: AbortController | undefined;
  let generation = 0;
  let insightRequest = 0;
  let eventRevision = -1;
  let disconnected = false;
  let pending = false;
  let notice = "";

  async function loadInsights(s: MatchSnapshot, ticket: number) {
    const request = ++insightRequest;
    try {
      const result = await port.matchInsights({ matchId: s.matchId, revision: s.revision });
      if (ticket !== generation || insightRequest !== request || snapshot?.revision !== s.revision) return;
      if (result.ok) insights = result.value;
      else if (result.error.code !== "revisionConflict") notice = matchError(result.error);
    } catch (error) {
      if (ticket !== generation || insightRequest !== request) return;
      notice = error instanceof Error ? error.message : String(error);
    }
    redraw();
  }
  function accept(s: MatchSnapshot, ticket: number) {
    if (ticket !== generation || s.accountId !== accountId || snapshot && s.revision <= snapshot.revision) return;
    snapshot = s;
    insights = undefined;
    redraw();
    return loadInsights(s, ticket);
  }
  async function reload(matchId: string, ticket: number) {
    const result = await port.getMatch({ matchId });
    if (ticket !== generation) return;
    if (!result.ok) throw new Error(matchError(result.error));
    if (snapshot?.revision === result.value.revision && !insights) await loadInsights(snapshot, ticket);
    else await accept(result.value, ticket);
  }
  async function follow(matchId: string, ticket: number, signal: AbortSignal) {
    try {
      const result = await port.subscribeMatch({ matchId }, signal);
      if (!result.ok) throw new Error(matchError(result.error));
      for await (const event of result.value) {
        if (signal.aborted || ticket !== generation) return;
        disconnected = false;
        if (event.kind === "snapshot") {
          if (event.snapshot.matchId !== matchId) continue;
          await accept(event.snapshot, ticket);
          redraw();
          continue;
        }
        if (event.matchId !== matchId || event.revision <= eventRevision) continue;
        eventRevision = event.revision;
        // A write response may arrive before its event. Keep the action in the log either way.
        if (event.kind === "actionApplied" && event.actor.kind === "model") {
          const history = histories.get(matchId) ?? [];
          histories.set(matchId, [...history.slice(-29), { revision: event.revision, action: event.action, modelId: event.actor.modelId }]);
          redraw();
        }
        if (!snapshot || event.revision > snapshot.revision) await reload(matchId, ticket);
      }
      if (!signal.aborted) throw new Error("对局推送已断开。");
    } catch (error) {
      if (signal.aborted || ticket !== generation) return;
      disconnected = true;
      notice = error instanceof Error ? error.message : String(error);
      redraw();
    }
  }
  function connect(matchId: string, ticket: number) {
    subscription?.abort();
    subscription = new AbortController();
    const { signal } = subscription;
    return follow(matchId, ticket, signal);
  }
  function reset(id?: string) {
    subscription?.abort();
    subscription = undefined;
    generation++;
    accountId = id;
    snapshot = undefined;
    insights = undefined;
    eventRevision = -1;
    pending = false;
    notice = "";
    disconnected = false;
  }
  async function write(work: () => Promise<void>) {
    if (pending) return;
    const ticket = generation;
    pending = true;
    notice = "";
    redraw();
    try { await work(); }
    catch (error) {
      if (ticket === generation) notice = error instanceof Error ? error.message : String(error);
    } finally {
      if (ticket === generation) {
        pending = false;
        redraw();
      }
    }
  }
  async function mutation(result: MatchResult<MatchSnapshot | null>, current: MatchSnapshot, ticket: number) {
    if (ticket !== generation) return;
    if (result.ok) {
      if (result.value) await accept(result.value, ticket);
      else await loadInsights(current, ticket);
      return;
    }
    if (result.error.code === "revisionConflict" || result.error.code === "controllerMismatch" ||
        result.error.code === "actionUnavailable") await reload(current.matchId, ticket);
    throw new Error(matchError(result.error));
  }
  async function start(id: string, operation: () => Promise<MatchResult<MatchSnapshot>>) {
    if (id !== accountId || active.has(id)) return;
    const ticket = generation;
    await write(async () => {
      const result = await operation();
      if (ticket !== generation) return;
      if (!result.ok) throw new Error(matchError(result.error));
      if (result.value.accountId !== id) throw new Error("对局服务返回了其他账号的局面。");
      active.set(id, result.value.matchId);
      await accept(result.value, ticket);
      if (ticket === generation) void connect(result.value.matchId, ticket);
    });
  }
  return {
    get snapshot() { return snapshot; },
    get insights() { return insights; },
    get history() { return snapshot ? histories.get(snapshot.matchId) ?? [] : []; },
    get disconnected() { return disconnected; },
    get pending() { return pending; },
    get notice() { return notice; },
    async enter(id: string) {
      if (id !== accountId) reset(id);
      const matchId = active.get(id);
      const ticket = generation;
      if (matchId) {
        await write(async () => {
          await reload(matchId, ticket);
          if (ticket === generation && (!subscription || disconnected)) void connect(matchId, ticket);
        });
      } else if (preview && !previewed.has(id)) {
        await start(id, () => preview(id));
        if (active.has(id)) previewed.add(id);
      }
    },
    select(id?: string) {
      if (id !== accountId) reset(id);
    },
    forget(id: string) {
      const matchId = active.get(id);
      if (matchId) histories.delete(matchId);
      active.delete(id);
      previewed.delete(id);
      if (id === accountId) reset();
    },
    create(input: CreateMatchInput) {
      return start(input.accountId, () => port.createMatch(input));
    },
    newMatch() {
      if (snapshot?.phase.kind !== "finished") return;
      const id = snapshot.accountId;
      active.delete(id);
      histories.delete(snapshot.matchId);
      reset(id);
      redraw();
    },
    reload() {
      const current = snapshot;
      const ticket = generation;
      if (!current) return;
      return write(async () => {
        await reload(current.matchId, ticket);
        if (ticket === generation && disconnected) void connect(current.matchId, ticket);
      });
    },
    execute(current: MatchSnapshot, entry: LegalAction) {
      if (snapshot !== current || current.controller.kind !== "manual" || disconnected) return;
      const ticket = generation;
      const { matchId, revision } = current;
      return write(async () => mutation(await port.executeAction({ matchId, revision, actionId: entry.actionId }), current, ticket));
    },
    control(current: MatchSnapshot, controller: Controller) {
      if (snapshot !== current) return;
      const ticket = generation;
      const { matchId, revision } = current;
      return write(async () => mutation(await port.setController({ matchId, revision, controller }), current, ticket));
    },
    feedback(current: MatchSnapshot, action: string, observed: Record<string, number>) {
      if (snapshot !== current || current.controller.kind !== "manual" || disconnected) return;
      const ticket = generation;
      const { matchId, revision } = current;
      return write(async () => mutation(await port.saveSessionFeedback({ matchId, revision, action, observed }), current, ticket));
    },
  };
}
export type MatchSession = ReturnType<typeof matchSession>;
