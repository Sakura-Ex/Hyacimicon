import { html, nothing } from "lit";
import { keyed } from "lit/directives/keyed.js";
import "./scroll-list";
import { art, icon, stats, type Spread, type Run } from "./folio";
import type { Account, AccountPort } from "./contracts";
import { accountImportView } from "./account-import";
import { accountManager } from "./account-manager";
import { imageUrl } from "./game-images";

export function accountView(
  port: AccountPort,
  redraw: () => void,
  run: Run,
  select: (a: Account) => void,
  remove: (id: string) => void,
) {
  let query = "",
    characterId = "",
    skillIndex = 1;
  let managing: ReturnType<typeof accountManager> | undefined;
  let adding: ReturnType<typeof accountImportView> | undefined;
  return (account: Account | undefined, accounts: Account[], warnings: string[]): Spread => {
    if (adding) return adding.render();
    if (managing) return managing(accounts, warnings);
    const characters = account?.characters ?? [];
    const visible = characters.filter((c) => c.name.includes(query.trim()));
    const c = visible.find((c) => c.id === characterId) ?? visible[0];
    const skill = c?.skills[skillIndex] ?? c?.skills[0];
    const change = (work: () => void) => {
      work();
      redraw();
    };
    const choose = (a: Account) => {
      query = "";
      characterId = "";
      skillIndex = 1;
      adding = undefined;
      select(a);
    };
    const add = async (initial?: { request: Record<string, unknown>; seed: string }) => {
      managing = undefined;
      const player = accountImportView(port, redraw, run, choose, () => change(() => (adding = undefined)), initial);
      adding = player;
      redraw();
      if (initial) await player.loadCatalogue();
    };
    const manage = () => change(() => {
      managing = accountManager(port, redraw, run, select, remove,
        () => change(() => (managing = undefined)),
        (request, seed) => add({ request, seed }), account?.id);
    });
    return {
      title: "角色档案",
      subtitle: "账号养成与装备",
      tools: html`<div class="account-tools">
        <span class="account-tools-avatar">${characters[0] ? art(characters[0].image, "thumb") : nothing}</span>
        <label
          ><select
            aria-label="切换账号"
            @change=${(e: Event) => {
              const a = accounts.find(
                (a) => a.id === (e.target as HTMLSelectElement).value,
              );
              if (a) choose(a);
            }}
          >
            ${accounts.map((a) => html`<option value=${a.id} .selected=${a.id === account?.id}>${a.name}</option>`)}</select
          ><small>${account?.source ?? "添加账号"}</small></label
        >
        <button @click=${manage}>管理<span class="account-tools-warning"
          ?hidden=${!warnings.length} aria-label="有文件读取提示">·</span></button>
        <button class="primary" @click=${() => add()}>${icon("plus")} 添加账号</button>
      </div>`,
      left: html`<div class="catalog">
        <aside>
          <h2>角色目录 <small>${characters.length} 名</small></h2>
          <label class="search"
            >${icon("search")}<input
              type="search"
              aria-label="搜索角色"
              placeholder="搜索角色名"
              .value=${query}
              @input=${(e: Event) => change(() => (query = (e.target as HTMLInputElement).value))}
          /></label>
          ${keyed(`${account?.id ?? ""}:${query.trim()}`, html`<scroll-list class="scroll-area" role="region" aria-label="角色列表" tabindex="0">
          <div class="roster">
            ${visible.map(
              (v) =>
                html`<button
                  class=${v.id === c?.id ? "selected" : ""}
                  @click=${() =>
                    change(() => {
                      characterId = v.id;
                      skillIndex = 1;
                    })}
                >
                  ${art(v.image)}${v.name}
                </button>`,
            )}
          </div></scroll-list>`)}
        </aside>
        ${
          c
            ? html`<article class="character scroll-area" tabindex="0" aria-label="角色资料">
                <b class="teal">${c.path}</b>
                <h1>${c.name}</h1>
                <p>
                  等级 ${c.level ?? "—"} · 突破 ${c.ascension ?? "—"} · 星魂
                  ${c.eidolons ?? "—"}
                </p>
                ${imageUrl(c.portrait) ? art(c.portrait, "portrait") : art(c.image, "unit-avatar")}
                <h2>基础属性</h2>
                ${c.stats.length ? stats(c.stats) : html`<p>暂无属性记录</p>`}
              </article>`
            : html`<p>${account && !characters.length ? "此账号暂无角色" : "没有匹配的角色"}</p>`
        }
      </div>`,
      right: html`${
        c
          ? html`<section>
                <h2>行迹</h2>
                <div class="skills">
                  <div>
                    ${[...new Set(c.skills.map((s) => s.group))].map(
                      (g) =>
                        html`<small>${g}</small>
                          <div class="chips">
                            ${c.skills.map((s, i) => (s.group === g ? html`<button class=${s === skill ? "primary" : ""} @click=${() => change(() => (skillIndex = i))}>${s.tab}</button>` : nothing))}
                          </div>`,
                    )}
                  </div>
                  ${
                    skill
                      ? html`<div class="skill-detail">
                          <b>${skill.tab} / 等级 ${skill.level ?? "—"}</b>
                          <h3>${skill.title ?? skill.tab + "名"}</h3>
                          <p>${skill.description || "暂无技能说明"}</p>
                          <div class="metrics">${stats(skill.scaling)}</div>
                        </div>`
                      : html`<p>暂无行迹记录</p>`
                  }
                </div>
              </section>
              <section>
                <h2>光锥</h2>
                ${
                  c.cone
                    ? html`<div class="cone">
                        ${art(c.cone.image)}
                        <div>
                          <small>${c.path.split("/").at(-1)}</small>
                          <h3>${c.cone.name}</h3>
                          <p>${c.cone.detail}</p>
                        </div>
                      </div>`
                    : html`<p>暂无光锥记录</p>`
                }
              </section>
              <section>
                <h2>遗器 <small>${c.relics.length} 件</small></h2>
                <div class="relics">
                  ${c.relics.map(
                    (i) =>
                      html`<div>
                        ${art(i.image)}<span
                          >${i.name}<small>${i.detail}</small></span
                        >
                      </div>`,
                  )}
                </div>
              </section>`
          : nothing
      }
`,
    };
  };
}
