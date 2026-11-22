import { html, nothing } from "lit";
import { art, icon, stats, type Spread, type Run } from "./folio";
import type { Account, PlayerPort } from "./contracts";
import { accountImportView } from "./account-import";

export function accountView(
  port: Pick<PlayerPort, "importAccount" | "generateAccount">,
  redraw: () => void,
  run: Run,
  select: (a: Account) => void,
) {
  let query = "",
    characterId = "",
    skillIndex = 1;
  let managing = false;
  let adding: (() => Spread) | undefined;
  return (account: Account | undefined, accounts: Account[]): Spread => {
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
      managing = false;
      adding = undefined;
      select(a);
    };
    if (adding) return adding();
    const add = () => change(() => {
      managing = false;
      adding = accountImportView(port, redraw, run, choose, () =>
        change(() => (adding = undefined)),
      );
    });
    return {
      title: "角色档案",
      subtitle: "账号养成与装备",
      tools: html`<div class="account-tools">
        ${characters[0] ? art(characters[0].image, "thumb") : nothing}
        <label
          ><select
            aria-label="切换账号"
            .value=${account?.id ?? ""}
            @change=${(e: Event) => {
              const a = accounts.find(
                (a) => a.id === (e.target as HTMLSelectElement).value,
              );
              if (a) choose(a);
            }}
          >
            ${accounts.map((a) => html`<option value=${a.id}>${a.name}</option>`)}</select
          ><small>${account?.source ?? "添加账号"}</small></label
        >
        <button @click=${() => change(() => (managing = true))}>管理</button
        ><button class="primary" @click=${add}>
          ${icon("plus")} 添加账号
        </button>
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
          </div>
        </aside>
        ${
          c
            ? html`<article class="character">
                <b class="teal">${c.path}</b>
                <h1>${c.name}</h1>
                <p>
                  等级 ${c.level ?? "—"} · 突破 ${c.ascension ?? "—"} · 星魂
                  ${c.eidolons ?? "—"}
                </p>
                  ${art(c.portrait ?? c.image, c.portrait ? "portrait" : "unit-avatar")}
                <h2>基础属性</h2>
                ${c.stats.length ? stats(c.stats) : html`<p>暂无属性记录</p>`}
              </article>`
            : html`<p>没有匹配的角色</p>`
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
      ${
        managing
          ? html`<dialog open aria-label="账号管理">
              <h2>管理账号</h2>
              ${accounts.map(
                (a) => html`<button @click=${() => choose(a)}>${a.name} · ${a.source}</button>`,
              )}
              <button @click=${() => change(() => (managing = false))}>关闭</button>
            </dialog>`
          : nothing
      }`,
    };
  };
}
