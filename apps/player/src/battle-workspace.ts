import { html, nothing } from "lit";
import { battleView } from "./battle";
import type {
  BattleCatalogue,
  BattleInput,
  BattlePort,
  BattleResult,
  Match,
  StandardBattle,
  Unit,
} from "./contracts";
import { art, itemArt, type Run, type Spread } from "./folio";

export function battleWorkspace(
  port: BattlePort,
  redraw: () => void,
  run: Run,
  currentMatch: () => Promise<Match>,
) {
  type Mode = BattleInput["mode"];
  function sessionFor(result: BattleResult) {
    const renderReplay = battleView(redraw);
    let selected = result.kind === "preview" ? result.units[0] : undefined;
    function view(): Spread {
      if (result.kind === "replay") return renderReplay(result.frames);
      return {
        title: "战斗准备",
        subtitle: "",
        tools: html``,
        left: html` ${
            selected
              ? html`<article class="battle-portrait">
                  <small>${selected.side}</small>
                  <h1>${selected.name}</h1>
                  ${art(selected.portrait ?? selected.image, selected.portrait ? "portrait" : "portrait avatar-portrait")}
                </article>`
              : nothing
          }
          <div class="deployed-units" aria-label="参战单位">
            ${result.units.map(
              (u) =>
                html`<button
                  aria-pressed=${u === selected}
                  @click=${() => {
                    selected = u;
                    redraw();
                  }}
                >
                  ${art(u.image)}${u.name}
                </button>`,
            )}
          </div>`,
        right: selected?.gear.length
          ? html`<section>
              <h2>已穿戴装备</h2>
              <div class="inventory">${selected.gear.map(itemArt)}</div>
            </section>`
          : html``,
      };
    }
    return { kind: result.kind, view };
  }
  let mode: Mode = "standard";
  let catalogue: BattleCatalogue | undefined;
  let standard:
    | { input: StandardBattle; session: ReturnType<typeof sessionFor> }
    | undefined;
  let currency:
    { match: Match; session: ReturnType<typeof sessionFor> } | undefined;
  let editing = false;

  async function enter(next: Mode = mode) {
    if (next === "standard" && !standard) {
      catalogue = await port.battleCatalogue();
      const input = catalogue.initial;
      standard = { input, session: sessionFor(await port.battle(input)) };
    }
    if (next === "currencyWars") {
      const match = await currentMatch();
      if (
        currency?.match.id !== match.id ||
        currency.match.revision !== match.revision
      )
        currency = {
          match,
          session: sessionFor(
            await port.battle({
              mode: next,
              matchId: match.id,
              revision: match.revision,
            }),
          ),
        };
    }
    mode = next;
    editing = false;
  }
  function choices(title: string, name: string, units: Unit[], ids: string[]) {
    return html`<fieldset>
      <legend>${title}</legend>
      ${units.map((u) => html`<label><input type="checkbox" name=${name} value=${u.id} .checked=${ids.includes(u.id)} /><img src=${u.image} alt="" /> ${u.name}</label>`)}
    </fieldset>`;
  }
  function setup(options: BattleCatalogue, input: StandardBattle) {
    return html`<form
      class="battle-setup"
      @submit=${(event: SubmitEvent) => {
        event.preventDefault();
        const values = new FormData(event.currentTarget as HTMLFormElement);
        const next: StandardBattle = {
          mode: "standard",
          allyIds: values.getAll("ally").map(String),
          enemyIds: values.getAll("enemy").map(String),
        };
        return run(async () => {
          if (!next.allyIds.length || !next.enemyIds.length)
            throw new Error("请至少选择一名角色和一名敌人。");
          if (next.allyIds.length > 4)
            throw new Error("常规战斗最多选择四名角色。");
          standard = {
            input: next,
            session: sessionFor(await port.battle(next)),
          };
          editing = false;
        });
      }}
    >
      <h2>阵容配置</h2>
      ${choices("我方角色", "ally", options.allies, input.allyIds)}
      ${choices("敌方单位", "enemy", options.enemies, input.enemyIds)}
      <footer>
        <button class="primary">应用阵容</button>
      </footer>
    </form>`;
  }
  function spread(): Spread {
    const session = mode === "standard" ? standard?.session : currency?.session;
    if (!session) throw new Error("尚未载入战斗。");
    const view = session.view();
    return {
      ...view,
      subtitle:
        mode === "standard" && standard
          ? `自选阵容 · ${standard.input.allyIds.length} 位角色 / ${standard.input.enemyIds.length} 名敌人`
          : "当前对局 · 节点 " + currency?.match.node,
      tools: html`<div class="battle-controls">
        <div role="group" aria-label="战斗模式">
          ${(["standard", "currencyWars"] as const).map((m) => html`<button aria-pressed=${mode === m} @click=${() => run(() => enter(m))}>${m === "standard" ? "常规战斗" : "货币战争"}</button>`)}
        </div>
        ${
          mode === "standard" && session.kind === "replay"
            ? html`<button
                class="setup-toggle"
                @click=${() => {
                  editing = !editing;
                  redraw();
                }}
              >
                ${editing ? "返回回放" : "配置队伍与敌人"}
              </button>`
            : nothing
        }
      </div>`,
      right:
        mode === "standard" &&
        (editing || session.kind === "preview") &&
        standard &&
        catalogue
          ? setup(catalogue, standard.input)
          : html`${view.right}
            ${
              mode === "currencyWars" && currency
                ? html`<section>
                    <h2>本局羁绊</h2>
                    <div class="traits">
                      ${currency.match.traits.map((t) => html`<div><img src=${t.image} alt="" /><strong>${t.name}</strong><span>${t.detail}</span></div>`)}
                    </div>
                  </section>`
                : nothing
            }`,
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
