import { html, nothing } from "lit";
import type { ActionOption, Match, MatchCommand, MatchItem, MatchPort, Piece } from "./contracts";
import { art, icon, stats, type Run, type Spread } from "./folio";
import { matchStatus } from "./match-status";

type Selection = { kind: "item" | "piece" | "event"; id: string };

function itemPicture(item: MatchItem) {
  const artwork = art(item.image);
  return artwork === nothing ? html`<span class="item-initial">${item.name.slice(0, 1)}</span>` : artwork;
}

function row(
  title: string,
  pieces: Piece[],
  capacity: number,
  inspect: (piece: Piece) => void,
  selected?: string,
  zone: "front" | "back" | "bench" = "bench",
) {
  return html`<section class="formation">
    <h2>${title} <small>${pieces.length} / ${capacity}</small></h2>
    <div class=${`slots ${zone}`} style=${`--slot-columns: ${Math.max(1, Math.min(capacity, 9))}`}>
      ${Array.from({ length: capacity }, (_, i) => {
        const p = pieces.find(piece => piece.slot === i);
        return p
          ? html`<button class="piece" type="button" aria-pressed=${selected === p.pieceId}
              aria-label=${`${p.name} · ${p.stars === null ? "星级未知" : p.stars + " 星"} · 装备 ${p.gear.length} 件`} @click=${() => inspect(p)}>
              <div class="piece-face">
                ${art(p.image)}
              </div>
              <strong title=${p.name}>${p.name}</strong>
              <span class="piece-stars" role="img" aria-label=${p.stars === null ? "星级未知" : `${p.stars} 星`}>
                ${p.stars === null ? "—" : "★".repeat(p.stars)}
              </span>
              <span class="piece-gear" aria-hidden="true">
                ${p.gear.length ? p.gear.map(itemPicture) : html`<span>—</span>`}
              </span>
            </button>`
          : html`<span class="empty" aria-label="空位">·</span>`;
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
  port: Pick<MatchPort, "executeMatchCommand" | "saveFeedback">,
  update: (m: Match) => void,
  redraw: () => void,
  run: Run,
  openBattle: (() => Promise<void>) | undefined,
  reload: () => Promise<void>,
) {
  let actionId = "";
  const inspections: { selection: Selection; revision: number; trigger: HTMLElement | null }[] = [];
  const inspect = (entry?: Selection, revision?: number) => {
    if (entry && revision !== undefined) {
      const trigger = document.activeElement as HTMLElement | null;
      if (!trigger?.closest(".match-inspector")) inspections.length = 0;
      inspections.push({ selection: entry, revision, trigger });
      redraw();
      document.getElementById("match-inspector-title")?.focus();
    } else {
      const closed = inspections.pop();
      redraw();
      if (closed?.trigger?.isConnected) closed.trigger.focus();
      else document.querySelector<HTMLElement>("#match-inspector-title, #match-resources")?.focus();
    }
  };
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
    const commands = m.commands;
    const pieces = [...m.front, ...m.back, ...m.bench];
    const allItems = [...m.inventory, ...m.consumables, ...pieces.flatMap(piece => piece.gear)];
    const inspection = inspections.at(-1);
    const current = inspection?.revision === m.revision ? inspection.selection : undefined;
    const selectedItem = current?.kind === "item" ? allItems.find(item => item.itemId === current.id) : undefined;
    const selectedPiece = current?.kind === "piece" ? pieces.find(piece => piece.pieceId === current.id) : undefined;
    const selectedEvent = current?.kind === "event" && m.event?.id === current.id ? m.event : undefined;
    const execute = (command: MatchCommand) => run(async () => {
      const next = await port.executeMatchCommand({ matchId: m.id, revision: m.revision, commandId: command.id });
      inspections.length = 0;
      update(next);
      document.getElementById("match-resources")?.focus();
    });
    const commandButton = (command: MatchCommand) => html`<button type="button" @click=${() => execute(command)}>
      <span><strong>${command.label}</strong>${command.detail ? html`<small>${command.detail}</small>` : nothing}</span>
      <span aria-hidden="true">›</span>
    </button>`;
    const itemCard = (item: MatchItem) => html`<button class="match-item" type="button"
        @click=${() => inspect({ kind: "item", id: item.itemId }, m.revision)}>
      <span class="item-picture">${itemPicture(item)}${item.count > 1 ? html`<small>×${item.count}</small>` : nothing}</span>
      <span>${item.name}</span>
    </button>`;
    const inspectPiece = (piece: Piece) => inspect({ kind: "piece", id: piece.pieceId }, m.revision);
    const lock = commands.find(command => command.operation.kind === "lockShop");
    const refresh = commands.find(command => command.operation.kind === "refreshShop");
    const choice = m.advice.find((a) => a.id === adviceId) ?? m.advice[0];
    const action = m.actions.find((a) => a.id === actionId) ?? m.actions[0];
    const open = (id: string) => {
      inspections.length = 0;
      editing = true;
      selectAction(id);
    };
    const inspector = selectedItem ?? selectedPiece ?? selectedEvent;
    const options = inspector ? commands.filter(({ operation }) => {
      if (selectedItem) {
        return ("itemId" in operation && operation.itemId === selectedItem.itemId) ||
          (operation.kind === "combine" && operation.otherItemId === selectedItem.itemId);
      }
      if (selectedPiece) {
        return (operation.kind === "move" || operation.kind === "sell") &&
          operation.pieceId === selectedPiece.pieceId;
      }
      return "eventId" in operation && operation.eventId === selectedEvent?.id;
    }) : [];
    return {
      title: "阵容部署",
      subtitle: m.preview ? "货币战争 · 演示对局" : "货币战争",
      tools: matchStatus(m, true),
      left: html`<div class="match-roster">
        ${row("前台", m.front, m.frontCapacity, inspectPiece, selectedPiece?.pieceId, "front")}
        ${row("后台", m.back, m.backCapacity, inspectPiece, selectedPiece?.pieceId, "back")}
        ${row("备战席", m.bench, m.benchCapacity, inspectPiece, selectedPiece?.pieceId)}
        <section class="match-traits">
          <h2>羁绊</h2>
          <div class="traits">
            ${m.traits.map((t) => html`<div>${art(t.image)}<strong>${t.name}</strong><span>${t.detail}</span></div>`)}
          </div>
        </section>
        </div>`,
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
          : html`<div class="match-content scroll-area" role="region" aria-label="对局操作" tabindex="0"
              @keydown=${(event: KeyboardEvent) => { if (event.key === "Escape" && inspector) inspect(); }}>
            ${inspector ? html`<section class="match-inspector">
                <button class="inspector-back" @click=${() => inspect()}>‹ 返回</button>
                <div class="inspector-heading">
                  ${selectedItem ? html`<span class="item-picture">${itemPicture(selectedItem)}</span>` : nothing}
                  ${selectedPiece ? art(selectedPiece.image) : nothing}
                  <div>
                    <h2 tabindex="-1" id="match-inspector-title">${"title" in inspector ? inspector.title : inspector.name}</h2>
                    ${"detail" in inspector && inspector.detail ? html`<p>${inspector.detail}</p>` : nothing}
                    ${selectedPiece && m.aura?.pieceId === selectedPiece.pieceId ? html`<p>${m.aura.name} · ${m.aura.detail}</p>` : nothing}
                  </div>
                </div>
                ${selectedPiece ? html`<h3>已装备 <small>${selectedPiece.gear.length} / 3</small></h3>
                  <div class="match-inventory">${selectedPiece.gear.map(itemCard)}</div>` : nothing}
                <div class="command-list">${options.length ? options.map(commandButton) : html`<p>当前没有可执行操作</p>`}</div>
              </section>` : html`
              <div class="metrics match-economy">
                ${stats([
                  ["金币", m.gold?.toString() ?? "—"],
                  ["预计利息", m.interest?.toString() ?? "—"],
                  ["连胜", m.streak?.toString() ?? "—"],
                  ["上阵人数", m.front.length + m.back.length + " / " + m.capacity],
                ])}
              </div>
              ${m.event || m.companion ? html`<section class="match-event">
                <div><strong>${m.event?.title ?? m.companion?.name}</strong><small>${m.event?.detail || m.companion?.detail}</small>
                  ${m.aura ? html`<small>${m.aura.name} · ${pieces.find(p => p.pieceId === m.aura?.pieceId)?.name ?? "主角"}</small>` : nothing}
                </div>
                ${m.event ? html`<button @click=${() => inspect({ kind: "event", id: m.event!.id }, m.revision)}>
                  ${m.event.kind === "aura" ? "选择光环" : "进入茶歇"} <span aria-hidden="true">›</span>
                </button>` : html`<span class="companion-label">奇遇伙伴</span>`}
              </section>` : nothing}
              <section class="match-stores">
                <h2 id="match-resources" tabindex="-1">装备 <small>${m.inventory.reduce((total, item) => total + item.count, 0)} 件待分配</small></h2>
                <div class="match-inventory">${m.inventory.length ? m.inventory.map(itemCard) : html`<p>暂无待分配装备</p>`}</div>
                <h2>消耗品</h2>
                <div class="match-inventory match-supplies">${m.consumables.length ? m.consumables.map(itemCard) : html`<p>暂无消耗品</p>`}</div>
              </section>
              <section class="match-shop">
                <div class="shop-heading"><h2>商店</h2><div class="shop-tools">
                  <button aria-pressed=${m.locked} ?disabled=${!lock} @click=${() => lock && execute(lock)}>
                    ${icon("lock")} ${lock?.label ?? (m.locked ? "已锁定" : "锁定")}
                  </button>
                  <button ?disabled=${!refresh} @click=${() => refresh && execute(refresh)}>
                    ${icon("cycle")} ${refresh?.label ?? "刷新"}
                  </button>
                </div></div>
                <div class="match-offers">${m.offers.map(offer => {
                  const command = commands.find(({ operation }) => operation.kind === "buy" && operation.offerId === offer.offerId);
                  return html`<button type="button" title=${offer.note} ?disabled=${!command}
                    aria-label=${command ? [command.label, command.detail].filter(Boolean).join(" · ") : offer.unit.name}
                    @click=${() => command && execute(command)}>
                    ${art(offer.unit.image)}<strong>${offer.unit.name}</strong>
                    <small>${icon("coin")}${offer.cost ?? "—"} <span aria-hidden="true">招募</span></small>
                  </button>`;
                })}${m.offers.length ? nothing : html`<p>暂无报价</p>`}</div>
                <div class="command-list">${commands.filter(({ operation }) => operation.kind === "buyExperience" || operation.kind === "advance").map(commandButton)}</div>
              </section>
              ${
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
                  : m.lastAction ? html`<p role="status">实际操作已记录</p>` : nothing
              }
              `}
              </div>
              <footer class="match-actions">
                <button class="match-reload" @click=${reload}>重读局面</button>
                ${openBattle ? html`<button @click=${openBattle}>查看本局战斗</button>` : nothing}
                ${action ? html`<button class="match-record" @click=${() => open(choice?.id ?? action.id)}>${icon("check")} 记录实际操作</button>` : nothing}
              </footer>`,
    };
  };
}
