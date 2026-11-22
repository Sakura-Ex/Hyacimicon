import { html } from "lit";
import { battleView } from "./battle";
import { matchStatus } from "./match-status";
import type { BattleInput, BattlePort, BattleResult, Match } from "./contracts";
import type { Run, Spread } from "./folio";

export function battleWorkspace(
  port: BattlePort,
  redraw: () => void,
  run: Run,
  currentMatch: () => Promise<Match>,
) {
  type Mode = BattleInput["mode"];
  function sessionFor(result: BattleResult) {
    const render = battleView(redraw);
    return () => render(result);
  }
  let standard: ReturnType<typeof sessionFor> | undefined;
  let currency:
    { match: Match; view: ReturnType<typeof sessionFor> } | undefined;
  async function enter(next: Mode = mode) {
    if (next === "standard" && !standard) {
      const { initial } = await port.battleCatalogue();
      standard = sessionFor(await port.battle(initial));
    }
    if (next === "currencyWars") {
      const match = await currentMatch();
      if (
        currency?.match.id !== match.id ||
        currency.match.revision !== match.revision
      ) {
        const result = await port.battle({
          mode: next,
          matchId: match.id,
          revision: match.revision,
        });
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
      subtitle: mode === "standard" ? "常规战斗" : currency?.match.preview ? "货币战争 · 演示对局" : "货币战争",
      tools: html`${mode === "currencyWars" && currency ? matchStatus(currency.match) : ""}
      <div class="battle-controls">
        <div role="group" aria-label="战斗模式">
          ${(["standard", "currencyWars"] as const).map((m) => html`<button aria-pressed=${mode === m} @click=${() => run(() => enter(m))}>${m === "standard" ? "常规战斗" : "货币战争"}</button>`)}
        </div>
        ${view.tools}
      </div>`,
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
