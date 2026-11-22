import { html, nothing } from "lit";
import type { Match, MatchStep } from "./contracts";

const kinds = {
  reward: { name: "奖励", mark: "·" },
  battle: { name: "战斗", mark: "×" },
  encounter: { name: "遭遇", mark: "◇" },
  boss: { name: "首领", mark: "◆" },
};

export function matchStatus(match: Match) {
  const planes = match.planes ?? [];
  const steps = planes.flat();
  const index = steps.findIndex((step) => step.id === match.node);
  const plane = planes.findIndex((nodes) => nodes.some((step) => step.id === match.node));
  const kind = steps[index]?.kind;
  const maximum = match.maxHealth;
  const health = match.health;
  const range = maximum != null && maximum > 0;

  function mark(step: MatchStep, before: boolean) {
    const current = step.id === match.node;
    const info = step.kind ? kinds[step.kind] : undefined;
    const label = [step.id, info?.name, current ? "当前节点" : before ? "已结算" : ""].filter(Boolean).join(" · ");
    return html`<li class=${before ? "past" : ""} title=${label}
      aria-label=${label} aria-current=${current ? "step" : nothing}>
      <span aria-hidden="true">${info?.mark ?? "·"}</span>
    </li>`;
  }

  let offset = 0;
  return html`<div class="match-status" role="region" aria-label="本局进度和整局血量">
    <div class="match-route">
      <div class="match-route-heading">
        <div>
          ${plane >= 0 ? html`<small>位面 ${plane + 1}</small>` : nothing}
          <strong>节点 ${match.node}</strong>
          ${kind ? html`<small>${kinds[kind].name}</small>` : nothing}
        </div>
        ${index >= 0 ? html`<span class="route-total">${index + 1}<small> / ${steps.length}</small></span>` : nothing}
      </div>
      ${planes.length ? html`<div class="match-planes">
        ${planes.map((nodes, i) => {
          const start = offset;
          offset += nodes.length;
          return html`<div class="match-plane">
            <ol aria-label=${"位面 " + (i + 1)}>${nodes.map((step, j) => mark(step, start + j < index))}</ol>
          </div>`;
        })}
      </div>` : nothing}
    </div>
    <div class="match-health">
      <small>整局血量</small>
      <div class="match-health-value"><strong>${health ?? "—"}</strong>${range ? html`<small> / ${maximum}</small>` : nothing}</div>
      ${range && health != null ? html`<meter min="0" max=${maximum} value=${health} aria-label="整局血量"></meter>` : nothing}
    </div>
  </div>`;
}
