import { html, nothing } from "lit";
import type { Account } from "./contracts.ts";
import type { Spread } from "./folio.ts";
import { appendObservation, decodeHistory, emptyObservation, observationFields, type Observation, type ObservationField, type RecordedObservation, type Zone } from "./observed-match.ts";

const leftFields: ObservationField[] = ["enemies", "affixes", "environment", "investments", "companion", "traits", "auras"];
const rightFields: ObservationField[] = ["version", "match", "node", "phase", "health", "maxHealth", "gold", "level", "experience", "streak", "freeRefreshes", "shopLock", "crystals", "items", "choices", "settlement", "action"];
const zoneLabels: Record<Zone, string> = { front: "前台", back: "后台", bench: "备战席" };

export function observedView(redraw: () => void, storage: Storage) {
  let selected: string | undefined;
  let draft = emptyObservation();
  let history: RecordedObservation[] = [];
  let notice = "";
  let readable = true;
  const key = () => `hyacimicon.observations.v1.${selected}`;
  function enter(account?: Account) {
    const id = account?.id ?? "unbound";
    if (id === selected) return;
    selected = id;
    draft = emptyObservation();
    history = [];
    notice = "";
    readable = true;
    try {
      history = decodeHistory(storage.getItem(key()));
      if (history.length) draft = structuredClone(history[history.length - 1].observation);
    } catch (cause) {
      readable = false;
      notice = `无法读取已有局面记录，请保留原始记录：${String(cause)}`;
    }
  }
  function save() {
    try {
      if (!readable) throw new Error("已有记录无法读取，暂不能覆盖。");
      const latest = decodeHistory(storage.getItem(key()));
      const next = appendObservation(latest, draft, new Date().toISOString());
      storage.setItem(key(), JSON.stringify(next));
      history = next;
      notice = `已记录第 ${history.length} 版实际局面`;
    } catch (cause) { notice = cause instanceof Error ? cause.message : String(cause); }
    redraw();
  }
  function text(label: string, value: string, update: (input: string) => void, multiline = false) {
    const change = (event: Event) => {
      const input = event.currentTarget;
      if (input instanceof HTMLInputElement || input instanceof HTMLTextAreaElement) update(input.value);
    };
    return html`<label class="observed-field"><span>${label}</span>${multiline
      ? html`<textarea rows="2" .value=${value} @input=${change} placeholder="未知可留空"></textarea>`
      : html`<input .value=${value} @input=${change} placeholder="待录入" />`}</label>`;
  }
  function field(id: ObservationField) {
    const label = observationFields.find(([key]) => key === id)?.[1] ?? id;
    return text(label, draft.fields[id], value => { draft.fields[id] = value; }, leftFields.includes(id) || ["crystals", "items", "choices", "settlement", "action"].includes(id));
  }
  return (account?: Account): Spread => {
    enter(account);
    return {
      title: "实战对局", subtitle: account?.name ?? "未绑定账号 · 可先记录局面",
      badge: `记录 ${history.length} 步`,
      tools: html`<button @click=${() => { draft = emptyObservation(); notice = "已清空编辑草稿，已保存记录保留。"; redraw(); }}>新对局</button><button class="primary" @click=${save} ?disabled=${!readable}>记录当前局面</button>`,
      left: html`<div class="observed-workspace scroll-area">
        <h2>阵容</h2><p class="observed-help">按游戏当前画面填写角色 ID 或名称、星级及已装备物品。空格留空。</p>
        ${(["front", "back", "bench"] as const).map(zone => {
          const slots = draft.board[zone];
          const label = zoneLabels[zone];
          return html`<h3>${label}</h3><div class="observed-slots" data-zone=${zone}>${slots.map((value, i) => text(`${i + 1}`, value, entry => { slots[i] = entry; }))}</div>`;
        })}
        <h2>局面概览</h2>${leftFields.map(field)}
      </div>`,
      right: html`<div class="observed-workspace scroll-area">
        <h2>商店</h2><div class="observed-shop">${draft.shop.map((value, i) => text(`报价 ${i + 1}`, value, entry => { draft.shop[i] = entry; }))}</div>
        <p class="observed-help">填写实际报价（角色、价格）；刷新后录入游戏提供的新报价。</p>
        <h2>进度与资源</h2><div class="observed-fields">${rightFields.map(field)}</div>
        <section class="advice"><h2>策略建议</h2><p>模型尚未接入，当前版本支持真实局面记录。</p></section>
        ${notice ? html`<p role="status">${notice}</p>` : nothing}
        ${history.length ? html`<details><summary>实战记录 · ${history.length} 步</summary><ol>${history.map(entry => html`<li>${entry.observation.fields.match} · ${entry.observation.fields.node} · ${entry.observation.fields.phase} · ${entry.observation.fields.action || "记录局面"}<button @click=${() => { draft = structuredClone(entry.observation); notice = `已载入第 ${entry.revision} 步为编辑草稿`; redraw(); }}>查看</button></li>`)}</ol></details>` : nothing}
      </div>`,
    };
  };
}
