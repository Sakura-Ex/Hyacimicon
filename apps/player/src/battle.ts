import { html, nothing } from "lit";
import type { Frame, Item } from "./contracts";
import { art, stats, type Spread } from "./folio";

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
  return (frames: Frame[]): Spread => {
    const f = frames[index];
    const actor = f?.units.find((u) => u.id === f.actorId);
    const unit = f?.units.find((u) => u.id === inspected) ?? actor;
    const step = (n: number) => {
      index += n;
      inspected = "";
      redraw();
    };
    const total = f?.targets.reduce((sum, t) => sum + t.damage, 0) ?? 0;
    const hasStats =
      unit &&
      (unit.hp !== null ||
        unit.energy !== null ||
        unit.stats.some(([, value]) => value !== "—"));
    const hasDetails =
      unit && (hasStats || unit.gear.length || unit.buffs.length);
    return {
      title: "战斗详情",
      subtitle: "独立战斗 / 行动回放",
      tools: html``,
      left:
        f && actor
          ? html`<div class="combat">
              <aside>
                <h2>行动轴 <small>行动值</small></h2>
                <div class="timeline">
                  ${f.units.map(
                    (u) =>
                      html`<button
                        class=${u.id === actor.id ? "active" : u.id === unit?.id ? "selected" : ""}
                        @click=${() => {
                          inspected = u.id;
                          redraw();
                        }}
                      >
                        ${art(u.image)}<strong
                          >${u.name}<small
                            >${u.id === actor.id ? "当前行动" : u.id === unit?.id ? "正在查看" : u.side}</small
                          ></strong
                        ><b>${u.initiative ?? "—"}</b>
                      </button>`,
                  )}
                </div>
              </aside>
              <article class="character">
                <b class="badge">当前行动</b>
                <h1>${actor.name}</h1>
                <p class="teal">${actor.element}</p>
                ${art(f.portrait ?? actor.image, "portrait")}
                <div class="frame-pager">
                  <div>
                    <small>行动回放</small
                    ><strong
                      >${index + 1} <small>/ ${frames.length}</small></strong
                    >
                  </div>
                  <button
                    aria-label="上一行动"
                    ?disabled=${index === 0}
                    @click=${() => step(-1)}
                  >
                    ‹
                  </button>
                  <button
                    aria-label="下一行动"
                    ?disabled=${index + 1 >= frames.length}
                    @click=${() => step(1)}
                  >
                    ›
                  </button>
                </div>
                ${f.skill ? html`<h3>${f.skill}</h3>` : nothing}
                ${f.targets.length ? html`<p>命中 ${f.targets.length} 名敌人 · 总伤害 ${total.toLocaleString()}</p>` : nothing}
                <div class="targets">
                  ${f.targets.map(
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
              </article>
            </div>`
          : html`<p>暂无战斗记录</p>`,
      right:
        unit && f && (hasDetails || inspected || f.log.length)
          ? html`<div class="inspected">
                ${art(unit.image)}
                <div>
                  <b>正在查看</b>
                  <h3>
                    ${unit.name} <small>${unit.side} ${unit.element}</small>
                  </h3>
                </div>
              </div>
              ${
                hasStats
                  ? html`<section class="unit-panel">
                      <h2>单位面板</h2>
                      <div class="meters">
                        ${meter("生命", unit.hp, unit.maxHp)}${meter("能量", unit.energy, unit.maxEnergy)}
                      </div>
                      <div class="metrics">${stats(unit.stats)}</div>
                    </section>`
                  : nothing
              }
              ${inspected && !hasDetails ? html`<p class="muted">暂无单位属性记录</p>` : nothing}
              ${unit.gear.length ? items("本场装备", unit.gear) : nothing}${unit.buffs.length ? items("状态效果", unit.buffs) : nothing}
              ${
                f.log.length
                  ? html`<section>
                      <h2>战斗记录</h2>
                      ${f.log.map(([time, event]) => html`<p class="log"><time>${time}</time>${event}</p>`)}
                    </section>`
                  : nothing
              }`
          : html`<p class="muted replay-note">
              暂无此行动的详细记录。
            </p>`,
    };
  };
}
