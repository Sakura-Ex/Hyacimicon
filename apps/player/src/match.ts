import { html, nothing } from "lit";
import type { ActionOption, Match, Piece, PlayerPort } from "./contracts";
import { art, icon, itemArt, stats, type Run, type Spread } from "./folio";

function row(
  title: string,
  pieces: Piece[],
  capacity: number,
  total: number,
  front = false,
) {
  return html`<section class="formation">
    <h2>${title} <small>${pieces.length} / ${capacity}</small></h2>
    <div class=${front ? "front slots" : "slots"}>
      ${Array.from({ length: total }, (_, i) => {
        const p = pieces[i];
        return p
          ? html`<div class="piece">
              ${art(p.image)}<strong
                >${p.name}<em
                  >${p.stars === null ? "" : "★".repeat(p.stars)}</em
                ></strong
              >
              <div class="worn">${p.gear.map(itemArt)}</div>
            </div>`
          : html`<span
              class=${i < capacity ? "empty" : "locked"}
              aria-label=${i < capacity ? "空位" : "未解锁"}
              >${i < capacity ? "＋" : icon("lock")}</span
            >`;
      })}
    </div>
  </section>`;
}
function feedback(
  m: Match,
  choice: ActionOption,
  choose: (id: string) => void,
  cancel: () => void,
  save: (e: SubmitEvent) => void,
) {
  return html`<form class="feedback" @submit=${save}>
    <b>节点 ${m.node} / 实战回填</b>
    <h3>记录实际操作</h3>
    <div class="scroll-area" role="region" aria-label="实战回填字段" tabindex="0">
    <section>
      <p class="feedback-row"><span>本步建议</span><strong>${m.advice[0]?.title ?? "暂无建议"}</strong></p>
      <label
        >实际操作<select
          .value=${choice.id}
          @change=${(e: Event) => choose((e.target as HTMLSelectElement).value)}
        >
          ${m.actions.map((a) => html`<option value=${a.id}>${a.title}</option>`)}
        </select></label
      >
    </section>
    <section>
      <h2>核对游戏内结果</h2>
      ${m.feedbackFields.map((f) => html`<label>${f.label}<input type="number" required step="1" name=${f.key} min=${f.min} max=${f.max ?? nothing} .value=${String(choice.expected[f.key] ?? f.current ?? "")} />${f.current === null ? nothing : html`<small>原为 ${f.current}</small>`}</label>`)}
    </section>
    </div>

    <footer>
      <button type="button" @click=${cancel}>返回</button
      ><button class="primary">${icon("check")} 保存记录</button>
    </footer>
  </form>`;
}
export function matchView(
  port: Pick<PlayerPort, "setShopLock" | "saveFeedback">,
  update: (m: Match) => void,
  redraw: () => void,
  run: Run,
  openBattle: () => Promise<void>,
) {
  let actionId = "";
  const selectAction = (id: string) => {
    actionId = id;
    redraw();
  };
  let adviceId = "",
    editing = false;
  const choose = (id: string) => {
    adviceId = id;
    redraw();
  };
  return (m: Match): Spread => {
    const refresh = m.refresh;
    const choice = m.advice.find((a) => a.id === adviceId) ?? m.advice[0];
    const action = m.actions.find((a) => a.id === actionId) ?? m.actions[0];
    const open = (id: string) => {
      editing = true;
      selectAction(id);
    };
    return {
      title: "阵容部署",
      subtitle: "节点 " + m.node,
      badge: "备战中",
      tools: html`<div class="metrics summary">
        ${stats([
          ["整局生命", m.health?.toString() ?? "—"],
          ["连胜", m.streak?.toString() ?? "—"],
          ["预计利息", m.interest?.toString() ?? "—"],
          ["团队规模", m.front.length + m.back.length + " / " + m.capacity],
        ])}
      </div>`,
      left: html`${row("前台", m.front, 4, 4, true)}${row("后台", m.back, m.backCapacity, 9)}${row("备战席", m.bench, 9, 9)}
        <section>
          <h2>羁绊</h2>
          <div class="traits">
            ${m.traits.map((t) => html`<div>${art(t.image)}<strong>${t.name}</strong><span>${t.detail}</span></div>`)}
          </div>
        </section>`,
      right:
        editing && action
          ? feedback(
              m,
              action,
              selectAction,
              () => {
                editing = false;
                redraw();
              },
              (e) => {
                e.preventDefault();
                const form = new FormData(e.target as HTMLFormElement);
                const observed = Object.fromEntries(
                  m.feedbackFields.map((f) => [f.key, Number(form.get(f.key))]),
                );
                return run(async () => {
                  const next = await port.saveFeedback({
                    matchId: m.id,
                    revision: m.revision,
                    action: action.id,
                    observed,
                  });
                  editing = false;
                  update(next);
                });
              },
            )
          : html`<div class="match-content scroll-area" role="region" aria-label="建议和商店" tabindex="0">${
                choice
                  ? html`<article class="advice">
                        <small>本步建议 <span>节点 ${m.node}</span></small>
                        <div>
                          ${choice.image ? art(choice.image, "thumb") : nothing}
                          <div>
                            <h3>${choice.title}</h3>
                            <p>${choice.reason}</p>
                          </div>
                        </div>
                        <div class="metrics">${stats(choice.effects)}</div>
                      </article>
                      <div class="alternatives">
                        <small>备选</small
                        >${m.advice.filter((a) => a !== choice).map((a) => html`<button @click=${() => choose(a.id)}>${a.title} <small>${a.reason}</small></button>`)}
                      </div>`
                  : html`<section>
                      <article class="advice">
                        <small>本步建议</small>
                        <h3>暂无建议</h3>
                      </article>
                      ${m.lastAction ? html`<p role="status">实际操作已记录</p>` : nothing}
                    </section>`
              }
              <div class="market">
                <div>
                  <section>
                    <h2>待分配装备</h2>
                    <div class="inventory">${m.inventory.map(itemArt)}</div>
                  </section>
                  <section>
                    <h2>消耗品</h2>
                    <div class="inventory">${m.consumables.map(itemArt)}</div>
                  </section>
                </div>
                <aside>
                  <h2>商店 <span>◎　${m.gold ?? "—"}</span></h2>
                  <div class="offers">
                    ${m.offers.map(
                      (o) =>
                        html`<div>
                          ${art(o.unit.image)}<strong
                            >${o.unit.name}<small>${o.note}</small></strong
                          ><b>◎　${o.cost ?? "—"}</b>
                        </div>`,
                    )}
                  </div>
                  <div class="shop-tools">
                    <button
                      aria-pressed=${m.locked}
                      @click=${() => run(async () => update(await port.setShopLock(m.id, m.revision, !m.locked)))}
                    >
                      ${icon("lock")} ${m.locked ? "已锁店" : "锁店"}
                    </button>
                    ${refresh ? html`<button @click=${() => open(refresh.actionId)}>${icon("cycle")} ${refresh.label}</button>` : nothing}
                  </div>
                </aside>
              </div>
              </div>
              <footer class="match-actions">
                <button @click=${openBattle}>查看本局战斗</button>
                ${action ? html`<button class="primary" @click=${() => open(choice?.id ?? action.id)}>${icon("check")} 记录实际操作</button>` : nothing}
              </footer>`,
    };
  };
}
