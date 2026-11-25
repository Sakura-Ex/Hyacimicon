import { html, nothing } from "lit";
import type { MatchSnapshot, NodeKind } from "./match-session-contract.ts";

const kinds = {
  reward: { name: "奖励", mark: "·" },
  battle: { name: "战斗", mark: "×" },
  encounter: { name: "遭遇", mark: "◇" },
  boss: { name: "首领", mark: "◆" },
  supply: { name: "补给", mark: "✦" },
} satisfies Record<NodeKind, { name: string; mark: string }>;

export function matchStatus(match: MatchSnapshot, showStepNumbers = false) {
  const { position, phase } = match;
  const plane = position?.nodeId.split("-")[0];
  const steps = plane ? match.route.filter(step => step.nodeId.startsWith(plane + "-")) : [];
  const resources = match.preparation?.resources;
  return html`<div class="match-status" role="region" aria-label="本局进度和整局血量">
    <div class="match-route">
      <div class="match-route-heading"><div>
        ${position ? html`<small>位面 ${plane}</small><strong>节点 ${position.nodeId}</strong><small>${
          phase.kind === "investmentStrategy" ? "策略待选 · 尚未入节点" : kinds[position.kind].name
        }</small>` : html`<strong>开局准备</strong><small>尚未进入 1-1</small>`}
      </div></div>
      ${steps.length && position ? html`<ol class="match-steps" aria-label=${`位面 ${plane} 的节点`}>
        ${steps.map((step, index) => {
          const current = step.nodeId === position.nodeId;
          const past = Number(step.nodeId.split("-")[1]) < Number(position.nodeId.split("-")[1]);
          const label = `${step.nodeId} · ${kinds[step.kind].name}${current ? " · 当前节点" : past ? " · 已通过" : ""}`;
          return html`<li class=${past ? "past" : ""} data-kind=${step.kind}
            title=${label + " · 已正式进入的常规节点 " + position.enteredTriggerNodes} aria-label=${label}
            aria-current=${current ? "step" : nothing}>
            <span aria-hidden="true">${showStepNumbers && step.kind !== "supply" ? index + 1 : kinds[step.kind].mark}</span>
          </li>`;
        })}
      </ol>` : nothing}
    </div>
    <div class="match-health">
      <small>整局血量</small>
      <div class="match-health-value"><strong>${resources?.health ?? "—"}</strong>${resources ? html`<small> / ${resources.maxHealth}</small>` : nothing}</div>
      ${resources ? html`<meter min="0" max=${resources.maxHealth} value=${resources.health} aria-label="整局血量"></meter>` : nothing}
    </div>
  </div>`;
}
