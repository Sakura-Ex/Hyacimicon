import { html, nothing, type TemplateResult } from "lit";
import { repeat } from "lit/directives/repeat.js";
import type { ActionOption, Match, MatchCommand, MatchItem, MatchPort, Piece } from "./contracts";
import { art, icon, stats, type Run, type Spread } from "./folio";
import { matchStatus } from "./match-status";
import { commandsForDrop, type DropTarget } from "./match-drag";

type Selection = { kind: "item" | "piece" | "event"; id: string; commandIds?: Set<string> };

function itemPicture(item: MatchItem) {
  const artwork = art(item.image);
  return artwork === nothing ? html`<span class="item-initial">${item.name.slice(0, 1)}</span>` : artwork;
}

function slots(
  pieces: Piece[],
  capacity: number,
  start: number,
  end: number,
  className: string,
  label: string,
  renderPiece: (piece: Piece) => TemplateResult,
) {
  return html`<div class=${`slots ${className}`} role="group" aria-label=${label}>
      ${Array.from({ length: end - start }, (_, offset) => {
        const i = start + offset;
        const p = pieces.find(piece => piece.slot === i);
        if (p) return renderPiece(p);
        return i < capacity ? html`<span class="empty" aria-label="空位"></span>`
          : html`<span class="empty reserved-slot" title="未解锁" aria-label="未解锁">${icon("lock")}</span>`;
      })}
    </div>`;
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
  let traitsExpanded = false;
  const setTraitsExpanded = (expanded: boolean) => {
    traitsExpanded = expanded;
    redraw();
    document.getElementById("match-traits-toggle")?.focus({ preventScroll: true });
  };
  let drag: { matchId: string; revision: number; itemId: string } | undefined;
  let dropHover: HTMLElement | undefined;
  const setDropHover = (target?: HTMLElement) => {
    if (target === dropHover) return;
    dropHover?.removeAttribute("data-drop-hover");
    dropHover = target;
    target?.setAttribute("data-drop-hover", "");
  };
  const leaveDrop = (event: DragEvent) => {
    if (dropHover !== event.currentTarget) return;
    if (event.relatedTarget instanceof Node && dropHover?.contains(event.relatedTarget)) return;
    setDropHover();
  };
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
      document.getElementById("match-resources")?.focus({ preventScroll: true });
    });
    const commandButton = (command: MatchCommand) => html`<button type="button" @click=${() => execute(command)}>
      <span><strong>${command.label}</strong>${command.detail ? html`<small>${command.detail}</small>` : nothing}</span>
      <span aria-hidden="true">›</span>
    </button>`;
    const draggedItemId = drag?.matchId === m.id && drag.revision === m.revision ? drag.itemId : undefined;
    const getDrop = (target: DropTarget) => drag?.matchId === m.id && drag.revision === m.revision
      ? commandsForDrop(commands, drag.itemId, target) : [];
    const allowDrop = (event: DragEvent, target: DropTarget) => {
      if (!getDrop(target).length) return;
      event.preventDefault();
      event.stopPropagation();
      setDropHover(event.currentTarget as HTMLElement);
      if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    };
    const drop = (event: DragEvent, target: DropTarget) => {
      const available = getDrop(target);
      if (!available.length || !drag) return;
      event.preventDefault();
      event.stopPropagation();
      const itemId = drag.itemId;
      drag = undefined;
      setDropHover();
      if (available.length === 1) {
        redraw();
        void execute(available[0]);
      } else {
        inspect({ kind: "item", id: itemId, commandIds: new Set(available.map(command => command.id)) }, m.revision);
      }
    };
    const itemCard = (item: MatchItem, pieceId?: string) => {
      const target = { itemId: item.itemId, pieceId };
      const draggable = !editing && commands.some(({ operation }) =>
        ("itemId" in operation && operation.itemId === item.itemId &&
          (operation.kind !== "useItem" || operation.pieceId !== undefined || operation.targetItemId !== undefined)) ||
        (operation.kind === "combine" && operation.otherItemId === item.itemId));
      const label = item.count > 1 ? `${item.name} ×${item.count}` : item.name;
      return html`<button class="match-item" type="button" title=${label} aria-label=${label}
        draggable=${String(draggable)} ?data-drop-ready=${getDrop(target).length > 0}
        ?data-drag-source=${draggedItemId === item.itemId}
        @dragstart=${(event: DragEvent) => {
          if (!draggable || !event.dataTransfer) return;
          event.dataTransfer.setData("application/x-hyacimicon-item", item.itemId);
          event.dataTransfer.effectAllowed = "move";
          const started = { matchId: m.id, revision: m.revision, itemId: item.itemId };
          drag = started;
          requestAnimationFrame(() => { if (drag === started) redraw(); });
        }}
        @dragend=${() => { drag = undefined; setDropHover(); redraw(); }}
        @dragover=${(event: DragEvent) => allowDrop(event, target)}
        @dragleave=${leaveDrop}
        @drop=${(event: DragEvent) => drop(event, target)}
        @click=${() => inspect({ kind: "item", id: item.itemId }, m.revision)}>
      <span class="item-picture">${itemPicture(item)}${item.count > 1 ? html`<small>×${item.count}</small>` : nothing}</span>
    </button>`;
    };
    const renderPiece = (p: Piece) => {
      const target = { pieceId: p.pieceId };
      const available = getDrop(target);
      const incoming = available.some(command => command.operation.kind === "equip")
        ? allItems.find(item => item.itemId === draggedItemId) : undefined;
      const label = `${p.name} · ${p.stars === null ? "星级未知" : p.stars + " 星"} · 装备 ${p.gear.length} 件`;
      return html`<div class="piece" ?data-drop-ready=${available.length > 0}
          @dragover=${(event: DragEvent) => allowDrop(event, target)}
          @dragleave=${leaveDrop}
          @drop=${(event: DragEvent) => drop(event, target)}>
        <button class="piece-main" type="button" aria-pressed=${selectedPiece?.pieceId === p.pieceId}
            aria-label=${label} title=${label} @click=${() => inspect({ kind: "piece", id: p.pieceId }, m.revision)}>
          <span class="piece-face">${art(p.image)}</span>
          <span class="piece-stars" aria-hidden="true">${p.stars === null ? "—" : "★".repeat(p.stars)}</span>
        </button>
        <div class="piece-gear">${repeat(p.gear, item => item.itemId, item => itemCard(item, p.pieceId))}</div>
        ${incoming ? html`<span class="incoming-gear" aria-hidden="true">${itemPicture(incoming)}</span>` : nothing}
      </div>`;
    };
    const lock = commands.find(command => command.operation.kind === "lockShop");
    const refresh = commands.find(command => command.operation.kind === "refreshShop");
    const buyExperience = commands.find(command => command.operation.kind === "buyExperience");
    const choice = m.advice.find((a) => a.id === adviceId) ?? m.advice[0];
    const action = m.actions.find((a) => a.id === actionId) ?? m.actions[0];
    const open = (id: string) => {
      inspections.length = 0;
      editing = true;
      selectAction(id);
    };
    const inspector = selectedItem ?? selectedPiece ?? selectedEvent;
    const options = inspector ? commands.filter(({ id, operation }) => {
      if (selectedItem) {
        if (current?.commandIds) return current.commandIds.has(id);
        return ("itemId" in operation && operation.itemId === selectedItem.itemId) ||
          (operation.kind === "combine" && operation.otherItemId === selectedItem.itemId);
      }
      if (selectedPiece) {
        return (operation.kind === "move" || operation.kind === "sell") &&
          operation.pieceId === selectedPiece.pieceId;
      }
      return "eventId" in operation && operation.eventId === selectedEvent?.id;
    }) : [];
    const traits = html`<aside class="match-traits" aria-label="阵容羁绊"
        @keydown=${(event: KeyboardEvent) => {
          if (event.key === "Escape" && traitsExpanded) setTraitsExpanded(false);
        }}>
      <div class="traits-panel" ?data-expanded=${traitsExpanded}>
        <div class="traits-heading">
          <h3>羁绊</h3>
          <button type="button" id="match-traits-toggle" aria-expanded=${traitsExpanded} aria-controls="match-trait-list"
            @click=${() => setTraitsExpanded(!traitsExpanded)}>${traitsExpanded ? "收起" : "展开"}</button>
        </div>
        <ul id="match-trait-list" class="match-trait-list" aria-label="羁绊列表" tabindex="0">
          ${m.traits.length ? m.traits.map(trait => {
            const progress = trait.progress;
            const active = progress?.thresholds.filter(value => value <= progress.count).at(-1);
            const artwork = art(trait.image);
            return html`<li title=${[trait.name, trait.detail].filter(Boolean).join(" · ")}>
              <span class="trait-emblem" ?data-active=${active !== undefined}>
                ${artwork === nothing ? trait.name.slice(0, 1) : artwork}
                ${progress ? html`<b aria-label=${`当前人数 ${progress.count}`}>${progress.count}</b>` : nothing}
              </span>
              <span class="trait-copy">
                <strong>${trait.name}</strong>
                ${progress?.thresholds.length ? html`<small class="trait-breakpoints" aria-label="激活档位">
                  ${progress.thresholds.map((value, index) => html`${index ? " / " : ""}<span ?data-active=${value === active}>${value}</span>`)}
                </small>` : nothing}
                ${traitsExpanded && trait.detail ? html`<small>${trait.detail}</small>` : nothing}
              </span>
            </li>`;
          }) : html`<li class="no-traits">暂无羁绊</li>`}
        </ul>
      </div>
    </aside>`;
    return {
      title: "阵容部署",
      subtitle: m.preview ? "货币战争 · 演示对局" : "货币战争",
      tools: matchStatus(m, true),
      left: html`<div class="match-roster">
        <div class="deploy-layout">
          <section class="formation front-formation">
            <h2>前台 <small>${m.front.length} / ${m.frontCapacity}</small></h2>
            ${slots(m.front, m.frontCapacity, 0, m.frontCapacity, "front", "前台", renderPiece)}
          </section>
          <section class="formation back-formation">
            <h2>后台 <small>${m.back.length} / ${m.backCapacity}</small></h2>
            ${slots(m.back, m.backCapacity, 0, 4, "back back-head", "后台首排", renderPiece)}
          </section>
          ${traits}
        </div>
        ${slots(m.back, m.backCapacity, 4, Math.max(9, m.backCapacity), "back back-tail", "后台续排", renderPiece)}
        <section class="formation bench-formation">
          <h2>备战席 <small>${m.bench.length} / ${m.benchCapacity}</small></h2>
          ${slots(m.bench, m.benchCapacity, 0, m.benchCapacity, "bench", "备战席", renderPiece)}
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
                  <div class="match-inventory">${repeat(selectedPiece.gear, item => item.itemId, item => itemCard(item, selectedPiece.pieceId))}</div>` : nothing}
                <div class="command-list">${options.length ? options.map(commandButton) : html`<p>当前没有可执行操作</p>`}</div>
              </section>` : html`
              <div class="metrics match-economy">
                ${stats([
                  ["金币", m.gold?.toString() ?? "—"],
                  ["预计利息", m.interest?.toString() ?? "—"],
                  ["连胜", m.streak?.toString() ?? "—"],
                  ["上阵人数", m.front.length + m.back.length + " / " + m.capacity],
                ])}
                <div class="match-training">
                  <small>${m.experience ? `Lv.${m.experience.level}${m.experience.required === null ? "" : ` · ${m.experience.current} / ${m.experience.required}`}` : "队伍等级"}</small>
                  <button ?disabled=${!buyExperience} title=${buyExperience?.detail ?? ""} @click=${() => buyExperience && execute(buyExperience)}>
                    ${buyExperience?.label ?? "购买经验"}
                  </button>
                </div>
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
                <div class="match-inventory stock-window" role="region" aria-label="待分配装备" tabindex="0">${m.inventory.length ? repeat(m.inventory, item => item.itemId, item => itemCard(item)) : html`<p>暂无待分配装备</p>`}</div>
                <h2>消耗品</h2>
                <div class="match-inventory match-supplies stock-window" role="region" aria-label="消耗品" tabindex="0">${m.consumables.length ? repeat(m.consumables, item => item.itemId, item => itemCard(item)) : html`<p>暂无消耗品</p>`}</div>
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
                  return html`<button type="button" title=${[offer.unit.name, offer.note].filter(Boolean).join(" · ")} ?disabled=${!command}
                    aria-label=${command ? [command.label, command.detail].filter(Boolean).join(" · ") : offer.unit.name}
                    @click=${() => command && execute(command)}>
                    ${art(offer.unit.image)}${offer.unit.image ? nothing : html`<span class="offer-initial" aria-hidden="true">${offer.unit.name.slice(0, 1)}</span>`}
                    <small>${icon("coin")}${offer.cost ?? "—"} <span aria-hidden="true">招募</span></small>
                  </button>`;
                })}${m.offers.length ? nothing : html`<p>暂无报价</p>`}</div>
                <div class="command-list">${commands.filter(({ operation }) => operation.kind === "advance").map(commandButton)}</div>
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
