import { html, nothing, type TemplateResult } from "lit";
import type { LegalAction, MatchAction } from "./match-session-contract.ts";
import type { MatchCatalogue } from "./match-catalogue.ts";
import { icon } from "./folio.ts";

export function matchControls(actions: readonly LegalAction[], catalogue: MatchCatalogue, execute: (entry: LegalAction) => void) {
  function find(kind: MatchAction["kind"], filter?: (action: MatchAction) => boolean) {
    return actions.find(entry => entry.action.kind === kind && (!filter || filter(entry.action)));
  }
  function button(entry: LegalAction | undefined, label: string, primary = false, pressed?: boolean) {
    return html`<button type="button" class=${primary ? "primary" : nothing} ?disabled=${!entry}
      aria-pressed=${pressed ?? nothing} @click=${() => entry && execute(entry)}>
      ${label}${entry?.goldCost ? html` · ${entry.goldCost} 金币` : nothing}
    </button>`;
  }
  return { actions, find, button, execute, label: catalogue.actionLabel };
}

export type MatchControls = ReturnType<typeof matchControls>;

export function disclosure(title: string, summary: TemplateResult, content: TemplateResult, alignEnd = false, group?: string) {
  return html`<details class="match-disclosure" name=${group ?? nothing} ?data-align-end=${alignEnd}
    @keydown=${(event: KeyboardEvent) => {
      const target = event.currentTarget;
      if (event.key === "Escape" && target instanceof HTMLDetailsElement && target.open) {
        event.stopPropagation();
        target.open = false;
        target.querySelector("summary")?.focus({ preventScroll: true });
      }
    }}>
    <summary>${summary}${icon("chev")}</summary>
    <div class="match-disclosure-panel edge-fade"><h3>${title}</h3>${content}</div>
  </details>`;
}

export function entryList(entries: { name: string; detail?: string }[], empty = "暂无条目") {
  return entries.length ? html`<ul class="match-overview-entries">${entries.map(entry =>
    html`<li><strong>${entry.name}</strong>${entry.detail ? html`<p>${entry.detail}</p>` : nothing}</li>`)}</ul>` : html`<p>${empty}</p>`;
}
