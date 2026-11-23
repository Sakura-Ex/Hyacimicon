import { html, nothing } from "lit";
import { keyed } from "lit/directives/keyed.js";
import { repeat } from "lit/directives/repeat.js";
import type { Account, ActionOption } from "./contracts.ts";
import { art, icon, stats, type Spread } from "./folio.ts";
import { auraList, board, itemCard, itemPicture, openingIntel, portrait, type BoardInteraction } from "./match-board.ts";
import { matchCatalogue, type MatchCatalogue } from "./match-catalogue.ts";
import { disclosure, entryList, matchControls, type MatchControls } from "./match-controls.ts";
import { actionsForDrop, type DragSource, type MatchDrop } from "./match-drag.ts";
import { phaseNames, phasePanel, settlement, setup, traitChoice } from "./match-phases.ts";
import type { LegalAction, MatchSnapshot, Preparation } from "./match-session-contract.ts";
import type { MatchSession } from "./match-session.ts";
import type { MatchInsights } from "./match-port.ts";
import { matchStatus } from "./match-status.ts";

type Inspection = (DragSource | { kind: "trait"; mechanism: string; id: string }) & {
  actionIds?: Set<string>;
};

function preparationView(s: MatchSnapshot, p: Preparation, c: MatchCatalogue, controls: MatchControls, ui: BoardInteraction,
  insights: MatchInsights | undefined, adviceId: string, chooseAdvice: (id: string) => void) {
  const { button, find } = controls;
  const r = p.resources;
  const interest = Math.min(r.interestCap, Math.floor(r.gold / 10));
  const active = p.board.pieces.filter(piece => piece.slot.zone !== "bench").length;
  const equipment = p.inventory.filter(item => c.item(item.definitionId).kind === "equipment");
  const consumables = p.inventory.filter(item => c.item(item.definitionId).kind === "consumable");
  const unclassified = p.inventory.filter(item => c.item(item.definitionId).kind === undefined);
  const choice = insights?.advice.find(a => a.id === adviceId) ?? insights?.advice[0];
  function stock(label: string, items: Preparation["inventory"]) {
    return html`<div class="match-inventory stock-window" role="region" aria-label=${label} tabindex="0">
      ${items.length ? repeat(items, item => item.itemId, item => itemCard(item, c, ui)) : html`<p>暂无${label}</p>`}</div>`;
  }
  return html`<div class="match-home">
    <div class="match-summary">
      <div class="metrics match-economy">
        ${stats([["上阵人数", active + " / " + p.board.active]])}
        <div class="match-training"><small>Lv.${r.level}${r.experienceToNextLevel === null ? nothing :
          html` · ${r.experience} / ${r.experienceToNextLevel}`}</small>
          ${button(find("buyExperience"), "购买经验")}</div>
      </div>
      ${s.companion ? html`<div class="match-event"><div><small>奇遇伙伴</small>
        <strong>${s.companion.name}</strong><small>${s.companion.detail}</small></div>
        ${s.phase.kind === "preparation" && s.phase.spotlightAvailable ?
          button(find("openSpotlight"), "选择主角光环 ›", true) : nothing}</div>` :
        s.phase.kind === "preparation" && s.phase.spotlightAvailable ? button(find("openSpotlight"), "选择主角光环 ›", true) : nothing}
    </div>
    <section class="match-stores">
      <div><h2 id="match-resources" tabindex="-1">装备 <small>${equipment.reduce((n, item) => n + item.count, 0)} 件待分配</small></h2>
        ${stock("待分配装备", equipment)}</div>
      <div class="match-supply-tray"><h2>消耗品</h2>${stock("消耗品", consumables)}</div>
      ${unclassified.length ? html`<div class="match-unclassified"><h2>待识别物品</h2>${stock("待识别物品", unclassified)}</div>` : nothing}
    </section>
    <section class="match-shop">
      <div class="shop-heading"><h2>商店</h2><div class="shop-tools">
        <div class="shop-balance">${disclosure("本局金币",
          html`${icon("coin")}<span>金币</span><strong>${r.gold}</strong>
            <span class="shop-income">火热连胜 <b>${r.hotStreak}</b> · 利息 <b>${interest}</b></span>`,
          entryList([{ name: "金币 " + r.gold }, { name: "火热连胜 " + r.hotStreak },
            { name: `预计利息 ${interest} / 上限 ${r.interestCap}` }]), true)}</div>
        ${button(find("lockShop"), p.shop.locked ? "解锁" : "锁定", false, p.shop.locked)}
        ${button(find("refreshShop"), p.shop.freeRefreshes > 0 ? `免费刷新 · ${p.shop.freeRefreshes}` : "刷新")}
      </div></div>
      <div class="match-offers">${Array.from({ length: 5 }, (_, index) => {
        const offer = p.shop.slots[index];
        if (!offer) return html`<div class="offer-empty" role="img" aria-label="商店空位"><span>空位</span></div>`;
        const unit = c.unit(offer.unitId);
        const traits = "traitIds" in unit && Array.isArray(unit.traitIds) ? unit.traitIds.map(c.trait) : [];
        const label = [unit.name, offer.price + " 金币", ...traits.map(trait => trait.name)].join(" · ");
        const entry = find("buy", a => a.kind === "buy" && a.offerId === offer.offerId);
        return html`<button type="button" title=${label} aria-label=${"招募 " + label}
          ?disabled=${!entry} @click=${() => entry && controls.execute(entry)}>
          <span class="offer-portrait" aria-hidden="true">
            <span class="offer-art">${portrait(unit)}</span>
            <span class="offer-traits">${traits.map(trait => html`<span class="offer-trait">
              ${art(trait.image, "offer-trait-icon")}<span class="offer-trait-name">${trait.name}</span></span>`)}</span>
          </span>
          <span class="offer-footer" aria-hidden="true"><span class="offer-name">${unit.name}</span>
            <span class="offer-price">${icon("coin")}${offer.price}</span></span>
        </button>`;
      })}</div>
    </section>
    ${choice ? html`<article class="advice">
      <small>本步建议 <span>节点 ${s.position?.nodeId}</span></small>
      <div>${choice.image ? art(choice.image, "thumb") : nothing}
        <div><h3>${choice.title}</h3><p>${choice.reason}</p></div></div>
      <div class="metrics">${stats(choice.effects)}</div></article>
      <div class="alternatives"><small>备选</small>${insights?.advice.filter(a => a !== choice).map(advice =>
        html`<button type="button" @click=${() => chooseAdvice(advice.id)}>${advice.title} <small>${advice.reason}</small></button>`)}</div>` : nothing}
    ${insights?.lastAction ? html`<p role="status">实际操作已记录</p>` : nothing}
  </div>`;
}

function feedback(s: MatchSnapshot, insights: MatchInsights, action: ActionOption, session: MatchSession,
  choose: (id: string) => void, close: () => void) {
  return html`<form class="feedback" @submit=${async (event: SubmitEvent) => {
    event.preventDefault();
    if (!(event.currentTarget instanceof HTMLFormElement)) return;
    const form = new FormData(event.currentTarget);
    const observed = Object.fromEntries(insights.feedbackFields.map(field => [field.key, Number(form.get(field.key))]));
    await session.feedback(s, action.id, observed);
    if (!session.notice) close();
  }}>
    <b>节点 ${s.position?.nodeId} / 实战回填</b><h3>记录实际操作</h3>
    <div class="scroll-area" role="region" aria-label="实战回填字段" tabindex="0">
      <section><p class="feedback-row"><span>本步建议</span><strong>${insights.advice[0]?.title ?? "暂无建议"}</strong></p>
      <label>实际操作<select @change=${(event: Event) => {
        if (event.target instanceof HTMLSelectElement) choose(event.target.value);
      }}>${insights.actions.map(entry => html`<option value=${entry.id} ?selected=${entry.id === action.id}>${entry.title}</option>`)}</select></label></section>
      <section><h2>核对游戏内结果</h2>${insights.feedbackFields.map(field =>
        html`<label>${field.label}<input type="number" required step="1" name=${field.key} min=${field.min} max=${field.max ?? nothing}
          .value=${String(action.expected[field.key] ?? field.current ?? "")}></label>`)}</section>
    </div>
    <footer><button type="button" @click=${close}>返回</button>
      <button class="primary" ?disabled=${session.pending || session.disconnected}>${icon("check")} 保存记录</button></footer>
  </form>`;
}

export function matchView(session: MatchSession, redraw: () => void, openBattle: (() => Promise<void>) | undefined, demoVersion?: string) {
  let last: MatchSnapshot | undefined;
  let expanded = false;
  let aura: { id: string; selectionId: string } | undefined;
  let editing: string | undefined;
  let adviceId = "";
  let drag: { source: DragSource; snapshot: MatchSnapshot } | undefined;
  let hover: HTMLElement | undefined;
  const inspections: { entry: Inspection; trigger: HTMLElement | null }[] = [];
  function highlight(target?: HTMLElement) {
    if (hover === target) return;
    hover?.removeAttribute("data-drop-hover");
    hover = target;
    hover?.setAttribute("data-drop-hover", "");
  }
  function inspect(entry: Inspection) {
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!trigger?.closest(".match-inspector")) inspections.length = 0;
    inspections.push({ entry, trigger });
    redraw();
    document.getElementById("match-inspector-title")?.focus({ preventScroll: true });
  }
  function closeInspection() {
    const previous = inspections.pop();
    redraw();
    if (previous?.trigger?.isConnected) previous.trigger.focus();
    else document.getElementById("match-inspector-title")?.focus();
  }

  return (account: Account | undefined): Spread => {
    const s = session.snapshot;
    if (s !== last) {
      if (s?.matchId !== last?.matchId) { expanded = false; aura = undefined; adviceId = ""; }
      inspections.length = 0;
      editing = undefined;
      drag = undefined;
      highlight();
      last = s;
    }
    const notice = session.notice ? html`<p class="session-notice" role="status">${session.notice}</p>` : nothing;
    if (!s) {
      const view = account ? setup(account, session.pending, demoVersion, session.create) :
        { left: html`<p>请先在账号书签中选择账号。</p>`, right: html`` };
      return { ...view, right: html`${notice}${keyed(account?.id, view.right)}`, title: "对局", subtitle: demoVersion ? "货币战争 · 演示对局" : "货币战争", tools: html`` };
    }

    const { phase, preparation: p, controller } = s;
    const insights = session.insights;
    const c = matchCatalogue(s, account, insights);
    const manual = controller.kind === "manual";
    const writeable = manual && !session.pending && !session.disconnected;
    const controls = matchControls(writeable ? s.legalActions : [], c, entry => { void session.execute(s, entry); });
    const { find, button, actions } = controls;
    const current = inspections.at(-1)?.entry;
    const selection = phase.kind === "spotlight" && phase.view === "selection" ? phase : undefined;
    const auraTarget = selection ? aura?.selectionId === selection.selectionId && selection.eligiblePieceIds.includes(aura.id)
      ? aura.id : selection.eligiblePieceIds[0] : undefined;
    function selectAura(id: string) {
      if (!selection?.eligiblePieceIds.includes(id)) return;
      aura = { id, selectionId: selection.selectionId };
      inspections.length = 0;
      redraw();
    }
    function closeFeedback() { editing = undefined; redraw(); }
    function dropActions(target: MatchDrop) {
      return drag && drag.snapshot === s ? actionsForDrop(actions, drag.source, target) : [];
    }
    const ui: BoardInteraction = {
      selected: current?.kind === "piece" ? current.id : undefined,
      auraTarget,
      inspectPiece(piece) {
        if (selection?.eligiblePieceIds.includes(piece.pieceId)) selectAura(piece.pieceId);
        else inspect({ kind: "piece", id: piece.pieceId });
      },
      inspectItem: item => inspect({ kind: "item", id: item.itemId }),
      inspectTrait: (mechanism, choice) => inspect({ kind: "trait", id: choice.choiceId, mechanism: mechanism.kind }),
      draggable(source) {
        return !editing && actions.some(({ action: a }) => source.kind === "piece" ?
          a.kind === "move" && a.pieceId === source.id :
          "itemId" in a && a.itemId === source.id || a.kind === "combine" && a.otherItemId === source.id);
      },
      dragStart(event, source) {
        if (!ui.draggable(source) || !event.dataTransfer) { event.preventDefault(); return; }
        event.dataTransfer.setData("application/x-hyacimicon-match", source.id);
        event.dataTransfer.effectAllowed = "move";
        const started = { source, snapshot: s };
        drag = started;
        requestAnimationFrame(() => { if (drag === started) redraw(); });
      },
      dragEnd() { drag = undefined; highlight(); redraw(); },
      dropReady: target => dropActions(target).length > 0,
      dropLabel(target) {
        const offered = dropActions(target);
        if (!offered.length || drag?.source.kind !== "item") return;
        const itemId = drag.source.id;
        const item = p?.inventory.find(item => item.itemId === itemId);
        const worn = p?.board.pieces.find(piece => piece.pieceId === target.pieceId)?.equipment;
        if (offered.some(entry => entry.action.kind === "combine") ||
            offered.some(entry => entry.action.kind === "equip") && item && c.item(item.definitionId).component &&
            worn?.some(item => c.item(item.definitionId).component)) return "合成";
        if (offered.some(entry => entry.action.kind === "equip")) return "装备";
        return "使用";
      },
      dragOver(event, target) {
        if (!dropActions(target).length || !(event.currentTarget instanceof HTMLElement)) return;
        event.preventDefault();
        event.stopPropagation();
        highlight(event.currentTarget);
        if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
      },
      dragLeave(event) {
        if (hover !== event.currentTarget || event.relatedTarget instanceof Node && hover?.contains(event.relatedTarget)) return;
        highlight();
      },
      drop(event, target) {
        const offered = dropActions(target);
        const source = drag?.source;
        if (!source || !offered.length) return;
        event.preventDefault();
        event.stopPropagation();
        drag = undefined;
        highlight();
        if (offered.length === 1) controls.execute(offered[0]);
        else inspect({ ...source, actionIds: new Set(offered.map(action => action.actionId)) });
      },
    };
    const piece = current?.kind === "piece" ? p?.board.pieces.find(piece => piece.pieceId === current.id) : undefined;
    const item = current?.kind === "item" ? [...p?.inventory ?? [],
      ...p?.board.pieces.flatMap(piece => piece.equipment) ?? []].find(item => item.itemId === current.id) : undefined;
    const choiceView = current?.kind === "trait" ? { mechanism: current.mechanism, choiceId: current.id } :
      phase.kind === "preparation" ? phase.openTraitChoice : null;
    const mechanism = p?.mechanisms.find(m => m.kind === choiceView?.mechanism);
    const choice = mechanism?.choices.find(c => c.choiceId === choiceView?.choiceId);
    const draft = choiceView && "draftOptionId" in choiceView ? choiceView.draftOptionId : undefined;
    const inspectionActions = actions.filter(({ action: a, actionId }) => {
      if (current?.actionIds) return current.actionIds.has(actionId);
      if (piece) return (a.kind === "move" || a.kind === "sell") && a.pieceId === piece.pieceId;
      return item && ("itemId" in a && a.itemId === item.itemId || a.kind === "combine" && a.otherItemId === item.itemId);
    });
    const unit = piece ? c.unit(piece.unitId) : undefined;
    const itemData = item ? c.item(item.definitionId) : undefined;
    const inspector = piece && unit || item && itemData ? html`<section class="match-inspector">
      <button type="button" class="inspector-back" @click=${closeInspection}>‹ 返回</button>
      <div class="inspector-heading">
        ${unit ? html`<span class="inspector-portrait">${portrait(unit)}</span>` : nothing}
        ${itemData ? itemPicture({ ...itemData, count: item?.count }) : nothing}
        <div><h2 tabindex="-1" id="match-inspector-title">${unit?.name ?? itemData?.name}</h2>${itemData?.detail ? html`<p>${itemData.detail}</p>` : nothing}</div>
      </div>
      ${piece && p ? html`<h3>已装备 <small>${piece.equipment.length} / 3</small></h3>
        <div class="match-inventory">${piece.equipment.map(item => itemCard(item, c, ui))}</div>
        ${p.selectedAuras.some(aura => aura.pieceId === piece.pieceId) ? auraList(p, c, piece.pieceId) : nothing}` : nothing}
      <div class="command-list">${inspectionActions.map(entry => button(entry, controls.label(entry.action)))}</div>
    </section>` : mechanism && choice ? html`<div class="match-inspector">
      ${current?.kind === "trait" ? html`<button type="button" @click=${closeInspection}>‹ 返回</button>` : nothing}
      ${traitChoice(mechanism, choice, controls, draft)}</div>` : nothing;

    const revisiting = "view" in phase && phase.view === "preparation";
    const workbench = p && (phase.kind === "preparation" || revisiting);
    const primary = phase.kind === "preparation" && !phase.openTraitChoice ? button(find("startBattle"), "开始战斗", true) :
      phase.kind === "battle" ? button(find("leaveBattle"), "暂离战斗", true) :
      phase.kind === "battleUnavailable" ? button(find("restartBattle"), "重试战斗", true) :
      phase.kind === "battleAway" ? button(find("reenterBattle"), "重新进入战斗", true) :
      phase.kind === "teaBreakReward" ? button(find("claimTeaBreakReward"), "领取奖励", true) :
      phase.kind === "finished" ? html`<button class="primary" type="button" @click=${session.newMatch}>开启新对局</button>` :
      "view" in phase ? button(find(revisiting ? "returnToSelection" : "returnToPreparation"),
        revisiting ? "回到选择" : "返回备战", true) : nothing;
    const action = insights?.actions.find(action => action.id === editing) ?? insights?.actions[0];
    const history = session.history;
    const headerControls = html`<div class="session-control" data-controller=${controller.kind}>
      ${controller.kind === "model" ? html`<span>模型接管 · ${controller.modelId}</span>
        <button type="button" class="primary" ?disabled=${session.pending} @click=${() => session.control(s, { kind: "manual" })}>收回控制</button>` :
        html`<span>手动控制</span>${phase.kind !== "finished" && insights?.models.length ?
          html`<details><summary>交给模型</summary><div>${insights.models.map(model =>
            html`<button type="button" ?disabled=${!writeable} @click=${() => session.control(s, { kind: "model", modelId: model.id })}>${model.name}</button>`)}</div></details>` : nothing}`}
      ${session.disconnected ? html`<button type="button" @click=${session.reload}>连接中断 · 重连</button>` : nothing}
    </div>`;
    return {
      title: "阵容部署",
      subtitle: demoVersion ? "货币战争 · 演示对局" : "货币战争",
      subtitleTools: headerControls,
      badge: phaseNames[phase.kind],
      tools: matchStatus(s, true),
      left: p ? board(s, p, c, controls, ui, expanded, value => { expanded = value; redraw(); }) : openingIntel(s),
      right: html`${notice}${editing && action && insights && manual ? feedback(s, insights, action, session,
        id => { editing = id; redraw(); }, closeFeedback) : html`
        <div class="match-content scroll-area" role="region" aria-label="对局操作" tabindex="0" @keydown=${(event: KeyboardEvent) => {
          if (event.key === "Escape" && current) closeInspection();
        }}>
          ${controller.kind === "model" || history.length ? html`<details class="settlement model-history">
            <summary>AI 操作流水${history.length ? html` · ${c.actionLabel(history.at(-1)?.action ?? history[0].action)}` : nothing}</summary>
            ${history.length ? html`<ol>${[...history].reverse().map(entry => html`<li>版本 ${entry.revision} · ${entry.modelId} · ${c.actionLabel(entry.action)}</li>`)}</ol>` : html`<p>等待模型操作。</p>`}
          </details>` : nothing}
          ${s.latestSettlement ? keyed(s.latestSettlement.nodeId, settlement(s.latestSettlement)) : nothing}
          ${revisiting ? html`<p class="phase-banner">${phaseNames[phase.kind]} ·
            ${phase.kind === "spotlight" ? "可调整站位与开启晶矿" : "可开启晶矿"}</p>` : nothing}
          ${inspector !== nothing ? inspector : workbench ?
            preparationView(s, p, c, controls, ui, insights, adviceId, id => { adviceId = id; redraw(); }) :
            phasePanel(s, c, controls, insights, auraTarget, selectAura)}
        </div>
        <footer class="match-actions">
          <button type="button" class="match-reload" ?disabled=${session.pending} @click=${session.reload}>重读局面</button>
          ${action && manual ? html`<button type="button" class="match-record" ?disabled=${!writeable} @click=${() => {
            inspections.length = 0;
            editing = insights?.advice.find(a => a.id === adviceId)?.id ?? insights?.advice[0]?.id ?? action.id;
            redraw();
          }}>${icon("check")} 记录实际操作</button>` : nothing}
          <span class="match-footer-spacer"></span>
          ${openBattle && p ? html`<button type="button" @click=${openBattle}>查看本局战斗</button>` : nothing}
          ${primary}
        </footer>`}`,
    };
  };
}
