import { html } from "lit";
import { battleView } from "./battle";
import { matchStatus } from "./match-status";
import type { BattleInput, BattlePort, BattleResult } from "./contracts";
import type { MatchSnapshot } from "./match-session-contract";
import type { Run, Spread } from "./folio";

export function battleWorkspace(
  port: BattlePort,
  redraw: () => void,
  run: Run,
  matchBattle: (snapshot: MatchSnapshot) => Promise<BattleResult>,
  currentMatch: () => MatchSnapshot | undefined,
) {
  type Mode = BattleInput["mode"];
  function sessionFor(result: BattleResult) {
    const render = battleView(redraw);
    return () => render(result);
  }
  let standard: ReturnType<typeof sessionFor> | undefined;
  let currency:
    { match: MatchSnapshot; view: ReturnType<typeof sessionFor> } | undefined;
  async function enter(next: Mode = mode) {
    if (next === "standard" && !standard) {
      const { initial } = await port.battleCatalogue();
      standard = sessionFor(await port.battle(initial));
    }
    if (next === "currencyWars") {
      const match = currentMatch();
      if (!match?.preparation) throw new Error("请先在对局中完成开局准备。");
      if (
        currency?.match.matchId !== match.matchId ||
        currency.match.revision !== match.revision
      ) {
        const result = await matchBattle(match);
        currency = { match, view: sessionFor(result) };
      }
    }
    mode = next;
  }
  let mode: Mode = "standard";
  function spread(): Spread {
    const render = mode === "standard" ? standard : currency?.view;
    if (!render) throw new Error("尚未载入战斗。");
    const view = render();
    return {
      ...view,
      subtitle: mode === "standard" ? "常规战斗" : "货币战争 · 演示对局",
      tools: mode === "currencyWars" && currency ? matchStatus(currency.match) : html``,
      right: html`
        <div class="battle-controls">
          <div role="group" aria-label="战斗模式">
            ${(["standard", "currencyWars"] as const).map((m) => html`<button aria-pressed=${mode === m}
              ?disabled=${m === "currencyWars" && !currentMatch()?.preparation}
              @click=${() => run(() => enter(m))}>${m === "standard" ? "常规战斗" : "货币战争"}</button>`)}
          </div>
          ${view.tools}
        </div>
        ${view.right}`,
    };
  }
  return {
    enter,
    spread,
    resetMatch() {
      currency = undefined;
      if (mode === "currencyWars") mode = "standard";
    },
  };
}
