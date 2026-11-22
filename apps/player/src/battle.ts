import { html, nothing } from "lit";
import type { BattleResult, Item } from "./contracts";
import { art, icon, stats, type Spread } from "./folio";

function meter(label: string, value: number | null, max: number | null) {
  return html`<label
    >${label}<b
      >${value?.toLocaleString() ?? "—"} / ${max?.toLocaleString() ?? "—"}</b
    >
    ${value !== null && max !== null ? html`<progress value=${value} max=${max}></progress>` : nothing}</label
  >`;
}
function items(title: string, entries: Item[]) {
  return html`<section>
    <h2>${title} <small>${entries.length} 项</small></h2>
    <div class="combat-items">
      ${entries.map(
        (i) =>
          html`<div>
            ${art(i.image)}<span
              ><b>${i.name}</b><small>${i.detail}</small></span
            >
          </div>`,
      )}
    </div>
  </section>`;
}
export function battleView(redraw: () => void) {
  let index = 0,
    inspected = "";
  return (result: BattleResult): Spread => {
    const frames = result.kind === "replay" ? result.frames : [];
    const f = frames[index];
    const units = result.kind === "preview" ? result.units : (f?.units ?? []);
    const actor = f ? units.find((u) => u.id === f.actorId) : undefined;
    const unit = units.find((u) => u.id === inspected) ?? units[1] ?? units[0];
    const featured = actor ?? units[0];
    const targets = f?.targets ?? [];
    const log = f?.log ?? [];
    const step = (n: number) => {
      index += n;
      inspected = "";
      redraw();
    };
    const total = f?.targets.reduce((sum, t) => sum + t.damage, 0) ?? 0;
    return {
      title: "战斗详情",
      subtitle: "独立战斗 / 行动回放",
      tools: html`<div class="battle-tools">
        <button ?disabled=${index === 0} @click=${() => step(-1)}>
          上一行动</button
        ><button
          ?disabled=${index + 1 >= frames.length}
          @click=${() => step(1)}
        >
          ${icon("step")} 下一行动
        </button>
      </div>`,
      left: html`<div class="combat">
        <aside>
          <h2>
            行动轴
            <small>${result.kind === "preview" ? "顺序待定" : "行动值"}</small>
          </h2>
          <div class="timeline">
            ${units.map(
              (u) =>
                html`<button
                  aria-pressed=${u.id === unit?.id}
                  class=${u.id === actor?.id ? "active" : u.id === unit?.id ? "selected" : ""}
                  @click=${() => {
                    inspected = u.id;
                    redraw();
                  }}
                >
                  ${art(u.image)}<strong
                    >${u.name}<small
                      >${u.id === actor?.id ? "当前行动" : u.id === unit?.id ? "正在查看" : u.side}</small
                    ></strong
                  ><b>${u.initiative ?? "—"}</b>
                </button>`,
            )}
          </div>
        </aside>
        ${
          featured
            ? html`<article class="character">
                <b class="badge">${f ? "当前行动" : "阵容预览"}</b>
                <h1>${featured.name}</h1>
                <p class="teal">${featured.element}</p>
                ${art(f?.portrait ?? featured.portrait ?? featured.image, "portrait")}
                <div class="damage">
                  <small>${f?.kind}</small>
                  <h3>${f?.skill}</h3>
                  <strong
                    >${targets.length ? total.toLocaleString() : "—"}</strong
                  >
                </div>
                <p>
                  ${targets.length ? `命中 ${targets.length} 名敌人` : "暂无命中记录"}
                </p>
                <div class="targets">
                  ${targets.map(
                    (t) =>
                      html`<div>
                        ${art(t.unit.image)}<span
                          >${t.unit.name}<b
                            >${t.damage.toLocaleString()}</b
                          ></span
                        >
                      </div>`,
                  )}
                </div>
              </article>`
            : html`<p>暂无战斗记录</p>`
        }
      </div>`,
      right: unit
        ? html`<div class="inspected">
              ${art(unit.image)}
              <div>
                <b>正在查看</b>
                <h3>
                  ${unit.name} <small>${unit.side} ${unit.element}</small>
                </h3>
              </div>
            </div>
            <div class="meters">
              ${meter("生命", unit.hp, unit.maxHp)}${meter("能量", unit.energy, unit.maxEnergy)}
            </div>
            <div class="metrics">${stats(unit.stats)}</div>
            ${items("本场装备", unit.gear)}${items("状态效果", unit.buffs)}
            <section>
              <h2>全场战斗记录</h2>
              ${log.length ? nothing : html`<p>暂无事件记录</p>`}${log.map(([time, event]) => html`<p class="log"><time>${time}</time>${event}</p>`)}
            </section>`
        : html``,
    };
  };
}
