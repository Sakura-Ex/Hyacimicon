import { html, nothing, type TemplateResult } from "lit";
import { repeat } from "lit/directives/repeat.js";
import type { Unit } from "./contracts.ts";
import type { ItemStack, MatchSnapshot, Piece, Preparation, Slot, TraitChoice, VisibleMechanism } from "./match-session-contract.ts";
import type { DragSource, MatchDrop } from "./match-drag.ts";
import { art, icon } from "./folio.ts";
import { crystalNames, mechanismNames, zoneNames, type MatchCatalogue } from "./match-catalogue.ts";
import { disclosure, entryList, type MatchControls } from "./match-controls.ts";

export function portrait(unit: Unit) {
  for (const source of [`/match-portraits/${unit.id}.webp`, unit.portrait, unit.image]) {
    const picture = art(source);
    if (picture !== nothing) return picture;
  }
  return html`<span class="portrait-initial" aria-hidden="true">${unit.name.slice(0, 1)}</span>`;
}
export function itemPicture(item: { name: string; image: string; count?: number }) {
  const picture = art(item.image);
  return html`<span class="item-picture">${picture === nothing ? html`<span class="item-initial">${item.name.slice(0, 1)}</span>` : picture}
    ${item.count && item.count > 1 ? html`<small>×${item.count}</small>` : nothing}</span>`;
}

export function auraList(p: Preparation, catalogue: MatchCatalogue, pieceId?: string) {
  return entryList(p.selectedAuras.filter(aura => pieceId === undefined || aura.pieceId === pieceId).map(aura => ({
    name: `${aura.name} · ${catalogue.pieceName(aura.pieceId)}`,
    detail: `${aura.effectActive ? "当前生效" : "历史光环"} · 难度 +${aura.difficultyIncrease}${aura.detail ? " · " + aura.detail : ""}`,
  })), "暂无主角光环");
}

function opponents(s: MatchSnapshot) {
  return html`${s.rival ? entryList([s.rival]) : nothing}
    ${s.opponents.map(plane => html`<h4>位面 ${plane.plane}</h4>
      ${entryList([...plane.enemies, ...plane.bosses.map(boss => ({ ...boss, name: "首领 · " + boss.name }))])}`)}`;
}

export function openingIntel(s: MatchSnapshot) {
  return html`<div class="session-intel"><section><h2>三位面对手</h2>
    <p>进入节点后揭示本场敌人。</p>${opponents(s)}</section>
    <section><h2>负面词条</h2>${entryList(s.affixes, "本局未选负面词条")}</section></div>`;
}

function overview(s: MatchSnapshot, p: Preparation, c: MatchCatalogue, controls: MatchControls) {
  const full = p.board.pieces.filter(piece => piece.slot.zone === "bench").length >= p.board.bench;
  const crystals = Object.entries(crystalNames).map(([quality, name]) => ({
    quality, name, count: p.crystals.filter(stack => stack.quality === quality).reduce((sum, stack) => sum + stack.count, 0),
  }));
  const panel = (title: string, summary: TemplateResult, body: TemplateResult, end = false) =>
    disclosure(title, summary, body, end, "match-overview");
  return html`<div class="match-info-strip" role="group" aria-label="对局概览">
    ${panel("敌人预览", html`<span>敌人</span><strong>${s.rival?.name ?? "待揭示"}</strong>`,
      html`<h4>当前节点 · 已揭示 ${p.revealedEnemies.length} 名</h4>
        ${entryList(p.revealedEnemies, "待揭示")}${opponents(s)}`)}
    ${panel("负面词条", html`<span>负面词条</span><strong>${s.affixes.length}</strong>`, entryList(s.affixes))}
    ${panel("投资环境", html`<span>投资环境</span><strong>${s.environment?.name ?? "待选"}</strong>`,
      entryList(s.environment ? [s.environment] : []))}
    ${panel("投资策略", html`<span>投资策略</span><strong>${p.selectedInvestments.length}</strong>`,
      html`${entryList(p.selectedInvestments)}${p.selectedAuras.length ? html`<h4>历次主角光环</h4>${auraList(p, c)}` : nothing}`)}
    <div class="match-crystals">${panel("晶矿", html`<span>晶矿</span>
      ${crystals.filter(crystal => crystal.count).map(crystal =>
        html`<span class="crystal-tally" title=${crystal.name + "色晶矿"} data-quality=${crystal.quality}>${crystal.count}</span>`)}`,
      html`${full ? html`<p>备战席已满，暂不可开启晶矿。</p>` : nothing}
        ${p.crystals.length ? html`<ul class="match-overview-entries">${p.crystals.map(crystal => {
          const action = controls.find("openCrystal", a => a.kind === "openCrystal" && a.crystalId === crystal.crystalId);
          return html`<li class="crystal-row"><strong>${crystalNames[crystal.quality]}色 ×${crystal.count}</strong>
            ${controls.button(action, "开启")}</li>`;
        })}</ul>` : html`<p>暂无晶矿</p>`}`, true)}</div>
  </div>`;
}

export interface BoardInteraction {
  selected?: string;
  auraTarget?: string;
  inspectPiece(piece: Piece): void;
  inspectItem(item: ItemStack): void;
  inspectTrait(mechanism: VisibleMechanism, choice: TraitChoice): void;
  draggable(source: DragSource): boolean;
  dragStart(event: DragEvent, source: DragSource): void;
  dragEnd(): void;
  dropReady(target: MatchDrop): boolean;
  dropLabel(target: MatchDrop): string | undefined;
  dragOver(event: DragEvent, target: MatchDrop): void;
  dragLeave(event: DragEvent): void;
  drop(event: DragEvent, target: MatchDrop): void;
}

export function itemCard(item: ItemStack, c: MatchCatalogue, ui: BoardInteraction) {
  const target = { itemId: item.itemId };
  const source: DragSource = { kind: "item", id: item.itemId };
  const data = c.item(item.definitionId);
  const label = item.count > 1 ? `${data.name} ×${item.count}` : data.name;
  return html`<button class="match-item" type="button" title=${label} aria-label=${label}
    draggable=${String(ui.draggable(source))} ?data-drop-ready=${ui.dropReady(target)}
    @click=${() => ui.inspectItem(item)} @dragstart=${(e: DragEvent) => ui.dragStart(e, source)}
    @dragend=${ui.dragEnd} @dragover=${(e: DragEvent) => ui.dragOver(e, target)}
    @dragleave=${ui.dragLeave} @drop=${(e: DragEvent) => ui.drop(e, target)}>
    ${itemPicture({ ...data, count: item.count })}
    ${ui.dropLabel(target) ? html`<span class="drop-caption" aria-hidden="true">${ui.dropLabel(target)}</span>` : nothing}
  </button>`;
}

export function board(s: MatchSnapshot, p: Preparation, c: MatchCatalogue, controls: MatchControls,
  ui: BoardInteraction, expanded: boolean, expand: (expanded: boolean) => void) {
  const eligible = s.phase.kind === "spotlight" && s.phase.view === "selection" ? s.phase.eligiblePieceIds : [];
  const activeAuras = new Set(p.selectedAuras.filter(aura => aura.effectActive).map(aura => aura.pieceId));
  function row(zone: Slot["zone"], start: number, end: number, classes: string) {
    return html`<div class=${"slots " + classes} role="group" aria-label=${zoneNames[zone]}>
      ${Array.from({ length: end - start }, (_, i) => {
        const index = i + start;
        const slot = { zone, index };
        const piece = p.board.pieces.find(piece => piece.slot.zone === zone && piece.slot.index === index);
        const target = { slot, pieceId: piece?.pieceId };
        const locked = index >= p.board[zone];
        const dropOver = (e: DragEvent) => ui.dragOver(e, target);
        const drop = (e: DragEvent) => ui.drop(e, target);
        if (!piece) return html`<span class=${"roster-slot empty" + (locked ? " reserved-slot" : "")}
          role="img" aria-label=${`${zoneNames[zone]} ${index + 1} · ${locked ? "未解锁" : "空位"}`}
          ?data-drop-ready=${ui.dropReady(target)} @dragover=${dropOver} @dragleave=${ui.dragLeave} @drop=${drop}>
            <span class="piece-main" aria-hidden="true"><span class="piece-face">${locked ? icon("lock") : nothing}</span></span>
            <span class="piece-gear" aria-hidden="true"></span></span>`;
        const unit = c.unit(piece.unitId);
        const source: DragSource = { kind: "piece", id: piece.pieceId };
        const label = `${unit.name} · ${piece.stars} 星 · 装备 ${piece.equipment.length} 件`;
        const caption = ui.dropLabel(target);
        return html`<div class="roster-slot piece" ?data-drop-ready=${ui.dropReady(target)}
          ?data-aura-eligible=${eligible.includes(piece.pieceId)}
          @dragover=${dropOver} @dragleave=${ui.dragLeave} @drop=${drop}>
          <button type="button" class="piece-main" title=${label} aria-label=${label}
            aria-pressed=${ui.selected === piece.pieceId || ui.auraTarget === piece.pieceId}
            draggable=${String(ui.draggable(source))}
            @dragstart=${(e: DragEvent) => ui.dragStart(e, source)} @dragend=${ui.dragEnd}
            @click=${() => ui.inspectPiece(piece)}>
            <span class="piece-face">${portrait(unit)}</span>
            <span class="piece-stars" aria-hidden="true">${"★".repeat(piece.stars)}</span>
            ${activeAuras.has(piece.pieceId) ? html`<span class="aura-mark" title="主角光环生效">✦</span>` : nothing}
          </button>
          <div class="piece-gear">${repeat(piece.equipment, item => item.itemId, item => itemCard(item, c, ui))}</div>
          ${caption ? html`<span class="drop-caption" aria-hidden="true">${caption}</span>` : nothing}
        </div>`;
      })}
    </div>`;
  }
  const size = (zone: Slot["zone"]) => `${p.board.pieces.filter(piece => piece.slot.zone === zone).length} / ${p.board[zone]}`;
  return html`${overview(s, p, c, controls)}<div class="match-roster">
    <div class="match-formations">
      <section class="formation front-formation"><h2>前台 <small>${size("front")}</small></h2>${row("front", 0, 4, "front")}</section>
      <section class="formation back-formation"><h2>后台 <small>${size("back")}</small></h2><div class="back-board">
        ${row("back", 0, 4, "back back-head")}${row("back", 4, 9, "back back-tail")}</div></section>
      <section class="formation bench-formation"><h2>备战席 <small>${size("bench")}</small></h2>${row("bench", 0, 9, "bench")}</section>
    </div>
    <aside class="match-traits" aria-label="阵容羁绊" @keydown=${(e: KeyboardEvent) => {
      if (e.key === "Escape" && expanded) {
        expand(false);
        document.getElementById("match-traits-toggle")?.focus({ preventScroll: true });
      }
    }}>
      <div class="traits-panel" ?data-expanded=${expanded}><div class="traits-heading"><h3>羁绊</h3>
        <button type="button" id="match-traits-toggle" aria-expanded=${expanded} aria-controls="match-trait-list"
          @click=${() => expand(!expanded)}>${expanded ? "收起" : "展开"}${icon("chev")}</button></div>
        <ul id="match-trait-list" class="match-trait-list" tabindex="0">
          ${p.traits.length ? p.traits.map(trait => {
            const info = c.trait(trait.traitId);
            const next = trait.thresholds.find(value => value > trait.count);
            const picture = art(info.image);
            return html`<li ?data-active=${trait.activeThreshold !== null} ?data-near=${next === trait.count + 1}
              title=${info.detail ? info.name + " · " + info.detail : info.name}>
              <span class="trait-emblem" aria-hidden="true">${picture === nothing ? info.name.slice(0, 1) : picture}</span>
              <span class="trait-copy"><strong>${info.name}</strong>${expanded ? html`
                <small class="trait-breakpoints" aria-label="激活档位">${trait.thresholds.map(value =>
                html`<span ?data-active=${value === trait.activeThreshold}>${value}</span>`)}</small>
                ${info.detail ? html`<small>${info.detail}</small>` : nothing}` : nothing}</span>
              <span class="trait-count" aria-label=${next === undefined ? `当前 ${trait.count} 人` : `当前 ${trait.count} 人，下一档 ${next} 人`}>
                <span>${trait.count}</span>${next === undefined ? nothing : html`<small>/</small><small>${next}</small>`}
              </span></li>`;
          }) : html`<li class="no-traits">暂无羁绊</li>`}
        </ul>
      </div>
      ${p.mechanisms.length ? html`<section class="trait-mechanisms" aria-label="羁绊机制" tabindex="0"><h4>羁绊机制</h4>
        ${p.mechanisms.map(m => html`<div class="mechanism-card"><strong>${mechanismNames[m.kind]}</strong>
          ${m.counters.map(counter => html`<small>${counter.id} ${counter.value}</small>`)}
          ${m.choices.map(choice => {
            const current = choice.options.find(option => option.optionId === choice.selectedOptionId);
            const open = controls.find("openTraitChoice", a => a.kind === "openTraitChoice" && a.mechanism === m.kind && a.choiceId === choice.choiceId);
            const once = controls.find("chooseMechanism", a => a.kind === "chooseMechanism" && a.mechanism === m.kind && a.choiceId === choice.choiceId);
            return html`<div class="mechanism-choice">${current ? html`<small>${current.name}</small>` : nothing}
              ${choice.mode === "changeable" ? controls.button(open, "修改") : once ?
                html`<button type="button" @click=${() => ui.inspectTrait(m, choice)}>选择</button>` : nothing}</div>`;
          })}
          ${controls.actions.filter(entry => entry.source?.mechanism === m.kind &&
            !["chooseMechanism", "openTraitChoice", "selectTraitOption", "confirmTraitChoice"].includes(entry.action.kind))
            .map(entry => controls.button(entry, controls.label(entry.action)))}
        </div>`)}
        ${controls.actions.filter(entry => entry.source?.kind === "trait" && entry.source.mechanism === undefined &&
          !["chooseMechanism", "openTraitChoice", "selectTraitOption", "confirmTraitChoice"].includes(entry.action.kind))
          .map(entry => controls.button(entry, c.trait(entry.source?.traitId ?? "").name + " · " + controls.label(entry.action)))}
      </section>` : nothing}
    </aside>
  </div>`;
}
