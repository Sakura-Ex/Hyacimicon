import { html, nothing } from "lit";
import type { Frame, Item } from "./contracts";
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
  return (frames: Frame[]): Spread => {
    const f = frames[index];
    const actor = f?.units.find((u) => u.id === f.actorId);
    const unit =
      f?.units.find((u) => u.id === inspected) ?? f?.units[1] ?? actor;
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
                <div class="damage">
                  <small>${f.kind}</small>
                  <h3>${f.skill}</h3>
                  <strong
                    >${f.targets.length ? total.toLocaleString() : "—"}</strong
                  >
                </div>
                <p>
                  ${f.targets.length ? `命中 ${f.targets.length} 名敌人` : "暂无命中记录"}
                </p>
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
        unit && f
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
                ${f.log.length ? nothing : html`<p>暂无事件记录</p>`}${f.log.map(([time, event]) => html`<p class="log"><time>${time}</time>${event}</p>`)}
              </section>`
          : html``,
    };
  };
}
