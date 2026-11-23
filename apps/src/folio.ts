import { html, nothing, type TemplateResult } from "lit";
import type { Stat } from "./contracts";
import { imageUrl } from "./game-images";
export interface Spread {
  title: string;
  subtitle: string;
  subtitleTools?: TemplateResult;
  tools: TemplateResult;
  left: TemplateResult;
  right: TemplateResult;
  badge?: string;
}
export const icon = (name: string) =>
  html`<img class="icon" alt="" draggable="false" src="/ui/${name}.svg" />`;
export function art(source: string | undefined, cls = "", alt = "") {
  const url = imageUrl(source);
  return url ? html`<img class=${cls} src=${url} alt=${alt} draggable="false" />` : nothing;
}
export const stats = (rows: Stat[]) =>
  rows.map(
    ([label, value]) =>
      html`<div class="stat">
        <span>${label}</span><strong>${value}</strong>
      </div>`,
  );
export type Run = (work: () => Promise<void>) => Promise<void>;
