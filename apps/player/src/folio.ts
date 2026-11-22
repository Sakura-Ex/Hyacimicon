import { html, nothing, type TemplateResult } from "lit";
import type { Item, Stat } from "./contracts";
export interface Spread {
  title: string;
  subtitle: string;
  tools: TemplateResult;
  left: TemplateResult;
  right: TemplateResult;
  badge?: string;
  layout?: "account-import";
}
export const icon = (name: string) =>
  html`<img class="icon" alt="" src="/ui/${name}.svg" />`;
export const art = (src: string, cls = "", alt = "") =>
  html`<img class=${cls} src=${src} alt=${alt} />`;
export const itemArt = (i: Item) =>
  html`<span class="item" tabindex="0" aria-label=${i.name + " " + i.detail}
    >${art(i.image)}
    ${i.count > 1 ? html`<small>×${i.count}</small>` : nothing}<span
      class="tooltip"
      >${[i.name, i.detail].filter(Boolean).join(" · ")}</span
    ></span
  >`;
export const stats = (rows: Stat[]) =>
  rows.map(
    ([label, value]) =>
      html`<div class="stat">
        <span>${label}</span><strong>${value}</strong>
      </div>`,
  );
export type Run = (work: () => Promise<void>) => Promise<void>;
