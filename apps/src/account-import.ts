import { html, nothing } from "lit";
import type { Account, AccountCatalogue, AccountPort, AccountSummary, ImportInspection, ImportReport } from "./contracts";
import type { Run, Spread } from "./folio";
import "./account-import.css";

const generationForm = "account-generation";
const importForm = "account-file-import";
const quality = ["", "低练", "基础", "成型", "高练", "毕业"];
const groups = ["limited", "standard", "free"] as const;
const groupLabels = { limited: "限定", standard: "常驻", free: "免费" };
const pickTargets = [
  ["core_character_ids", "核心角色"],
  ["character_pool", "角色池"],
  ["light_cone_pool", "光锥池"],
] as const;
type PickTarget = (typeof pickTargets)[number][0];

export function inventorySummary(summary: AccountSummary) {
  return html`<dl class="account-metrics">
    <div><dt>角色</dt><dd>${summary.characters}</dd></div>
    <div><dt>光锥</dt><dd>${summary.lightCones}<small>闲置 ${summary.idleLightCones}</small></dd></div>
    <div><dt>遗器</dt><dd>${summary.relics}<small>闲置 ${summary.idleRelics}</small></dd></div>
  </dl>`;
}

export function importReportView(report: ImportReport, showPassed = true) {
  return html`
    <p class="account-provenance">Fribbels v${report.format_version} · 导出器 ${report.build}</p>
    ${report.warnings.length ? html`<details class="account-warnings">
      <summary>查看 ${report.warnings.length} 项导入提示</summary>
      <ul>${report.warnings.map(warning => html`<li><code>${warning.path}</code> ${warning.message}</li>`)}</ul>
    </details>` : showPassed ? html`<p class="account-provenance">校验通过</p>` : nothing}
  `;
}

function object(value: unknown, message: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error(message);
  return { ...value };
}

function ids(value: string) {
  return [...new Set(value.trim().split(/[\s,，]+/).filter(Boolean))];
}

async function readUtf8(file: File): Promise<string> {
  const bytes = await file.arrayBuffer();
  try {
    return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch (cause) {
    throw new Error("文件应使用 UTF-8 编码。", { cause });
  }
}

export function accountImportView(
  port: AccountPort,
  redraw: () => void,
  run: Run,
  select: (account: Account) => void,
  dismiss: () => void,
  initial?: { request: Record<string, unknown>; seed: string },
  research = true,
) {
  let mode: "import" | "generate" = initial ? "generate" : "import";
  let modeForm: "fields" | "json" = initial ? "json" : "fields";
  let source: { fileName: string; json: string; inspection: ImportInspection } | undefined;
  let importName = "";
  let seed = initial?.seed ?? "";
  let name = "";
  let error = "";
  let pending = false;
  let dragging = false;
  let catalogue: AccountCatalogue | undefined;
  let catalogueError = "";
  let pickTarget: PickTarget = "core_character_ids";
  let pickSearch = "";
  let requestText = initial ? JSON.stringify(initial.request, null, 2) : "";
  const fields: Record<string, string> = {};
  const goldCategories = new Set<string>(["limited", "standard"]);

  async function loadCatalogue() {
    if (catalogue) return;
    try {
      catalogue = await port.accountCatalogue();
      catalogueError = "";
    } catch (cause) {
      catalogueError = cause instanceof Error ? cause.message : String(cause);
    }
    redraw();
  }

  function close() {
    if (!pending) dismiss();
  }

  async function changeMode(next: typeof mode) {
    if (pending || mode === next || (!research && next === "generate")) return;
    mode = next;
    error = "";
    redraw();
    if (next === "generate") await loadCatalogue();
  }

  async function readFile(files: FileList) {
    const file = files.item(0);
    if (!file || pending) return;
    dragging = false;
    source = undefined;
    if (files.length !== 1 || !/\.json$/i.test(file.name)) {
      error = "请选择一个账号 JSON 文件。";
      redraw();
      return;
    }
    await run(async () => {
      pending = true;
      error = "";
      redraw();
      try {
        const json = await readUtf8(file);
        const inspection = await port.inspectAccount(file.name, json);
        source = { fileName: file.name, json, inspection };
        importName = inspection.name;
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
      } finally {
        pending = false;
      }
    });
  }

  function requestFromFields(): Record<string, unknown> {
    const request: Record<string, unknown> = { gold_categories: [...goldCategories] };
    for (const key of ["character_count", "gold_count", "core_count", "resource_concentration",
      "gold_concentration", "relic_concentration", "teammate_bias", "light_cone_gold_share",
      "min_mean_effective_rolls", "max_mean_effective_rolls", "relic_quality_tier", "max_attempts"]) {
      const value = fields[key];
      if (value) request[key] = Number(value);
    }
    for (const key of ["character_pool", "core_character_ids", "light_cone_pool"]) {
      if (fields[key]?.trim()) request[key] = ids(fields[key]);
    }
    if (fields.account_id?.trim()) request.account_id = fields.account_id.trim();
    if (fields.stars && fields.cone) request.five_star_build = [Number(fields.stars), Number(fields.cone)];
    else if (fields.stars || fields.cone) throw new Error("统一五星配装需要同时设置星魂和专锥叠影。");
    if (fields.character_overrides?.trim()) {
      request.character_overrides = object(JSON.parse(fields.character_overrides), "角色覆盖应为按角色 ID 索引的 JSON 对象。");
    }
    return request;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (pending) return;
    const file = source;
    if (mode === "import" && !file) return;
    await run(async () => {
      pending = true;
      error = "";
      redraw();
      try {
        if (mode === "import" && file) {
          select(await port.importAccount(file.fileName, file.json, importName));
        } else {
          const request = modeForm === "fields" ? requestFromFields() : object(JSON.parse(requestText), "生成请求应为 JSON 对象。");
          select(await port.generateAccount(request, seed.trim() || null, name.trim() || null));
        }
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
      } finally {
        pending = false;
      }
    });
  }

  async function readRequest(file: File) {
    try {
      const json: unknown = JSON.parse((await readUtf8(file)).replace(/^\uFEFF/, ""));
      const saved = object(json, "生成请求应为 JSON 对象。");
      const request = saved.request === undefined ? saved : object(saved.request, "请求文件中的 request 应为 JSON 对象。");
      if (saved.request !== undefined && saved.seed !== undefined) {
        if (typeof saved.seed !== "string" || !/^\d{1,20}$/.test(saved.seed)) {
          throw new Error("请求文件中的种子应为十进制字符串。");
        }
        seed = saved.seed;
      }
      requestText = JSON.stringify(request, null, 2);
      modeForm = "json";
      error = "";
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
    redraw();
  }

  const number = (key: string, label: string, min: number, step: number | "any", options: {
    max?: number; placeholder?: string; required?: boolean;
  } = {}) => html`<label class="account-field">${label}
    <input form=${generationForm} type="number" min=${min} step=${step}
      max=${options.max ?? nothing} placeholder=${options.placeholder ?? nothing}
      ?required=${!!options.required} ?disabled=${pending} .value=${fields[key] ?? ""}
      @invalid=${(event: Event) => {
        const group = event.currentTarget instanceof Element ? event.currentTarget.closest("details") : null;
        if (group instanceof HTMLDetailsElement) group.open = true;
      }}
      @input=${(event: Event) => {
        if (event.currentTarget instanceof HTMLInputElement) fields[key] = event.currentTarget.value;
      }} />
  </label>`;

  const textField = (key: string, label: string, placeholder: string) => html`<label class="account-field">${label}
    <input form=${generationForm} type="text" .value=${fields[key] ?? ""} placeholder=${placeholder}
      ?disabled=${pending} @input=${(event: Event) => {
        if (event.currentTarget instanceof HTMLInputElement) fields[key] = event.currentTarget.value;
      }} />
  </label>`;

  const choice = (key: string, label: string, choices: number[]) => html`<label class="account-field">${label}
    <select form=${generationForm} ?disabled=${pending} @change=${(event: Event) => {
      if (event.currentTarget instanceof HTMLSelectElement) fields[key] = event.currentTarget.value;
    }}>
      <option value="" .selected=${!fields[key]}>自动</option>
      ${choices.map(value => html`<option value=${value} .selected=${fields[key] === String(value)}>${value}</option>`)}
    </select>
  </label>`;

  const generationFields = () => html`
    <fieldset class="account-group">
      <legend>五星统一配装</legend>
      <div class="account-fields">${choice("stars", "星魂", [0,1,2,3,4,5,6])}${choice("cone", "专锥叠影", [1,2,3,4,5])}</div>
    </fieldset>
    <fieldset class="account-group">
      <legend>遗器配置</legend>
      <label class="account-field">整体质量档位
        <select form=${generationForm} ?disabled=${pending} @change=${(event: Event) => {
          if (event.currentTarget instanceof HTMLSelectElement) fields.relic_quality_tier = event.currentTarget.value;
        }}>
          ${quality.map((label, index) => html`<option value=${index || ""} .selected=${Number(fields.relic_quality_tier || 0) === index}>${label || "自动"}</option>`)}
        </select>
      </label>
      <div class="account-fields">
        ${number("min_mean_effective_rolls", "平均有效词条下限", 0, "any", { placeholder: "不限" })}
        ${number("max_mean_effective_rolls", "平均有效词条上限", 0, "any", { placeholder: "不限" })}
      </div>
    </fieldset>`;

  function cataloguePicker(data: AccountCatalogue) {
    const selected = new Set(ids(fields[pickTarget] ?? ""));
    const entries = pickTarget === "light_cone_pool" ? data.lightCones : data.characters;
    const query = pickSearch.trim();
    const results = entries.filter(entry => entry.name.includes(query) || entry.id.includes(query));
    return html`<details class="account-catalogue">
      <summary>从目录挑选</summary>
      <div class="account-catalogue-content">
        <label class="account-field">添加到
          <select ?disabled=${pending} @change=${(event: Event) => {
            if (!(event.currentTarget instanceof HTMLSelectElement)) return;
            const value = event.currentTarget.value;
            if (value !== "core_character_ids" && value !== "character_pool" && value !== "light_cone_pool") return;
            pickTarget = value;
            pickSearch = "";
            redraw();
          }}>
            ${pickTargets.map(([key, label]) =>
              html`<option value=${key} .selected=${key === pickTarget}>${label}</option>`)}
          </select>
        </label>
        <label class="account-field">搜索名称或 ID
          <input type="search" .value=${pickSearch} ?disabled=${pending} @input=${(event: Event) => {
            if (!(event.currentTarget instanceof HTMLInputElement)) return;
            pickSearch = event.currentTarget.value;
            redraw();
          }} />
        </label>
        <scroll-list class="account-candidates-scroll scroll-area" role="region" aria-label="生成候选列表" tabindex="0"><div class="account-candidates">
          ${results.length ? results.map(entry => html`<button type="button" ?disabled=${pending}
            aria-pressed=${selected.has(entry.id)}
            @click=${() => {
              if (selected.has(entry.id)) selected.delete(entry.id); else selected.add(entry.id);
              fields[pickTarget] = [...selected].join(", ");
              redraw();
            }}>${entry.name} <small>${entry.id}</small></button>`)
            : html`<p>没有匹配的条目。</p>`}
        </div></scroll-list>
      </div>
    </details>`;
  }

  const advanced = () => html`
    <details class="account-fold">
      <summary>角色与光锥候选</summary>
      <div class="account-fields account-core-fields">
        ${textField("core_character_ids", "指定核心角色 ID", "以逗号或空格分隔")}
        ${number("core_count", "自动核心数量", 0, 1, { placeholder: "默认 1" })}
      </div>
      ${textField("character_pool", "角色池 ID", "留空使用全部候选")}
      ${textField("light_cone_pool", "光锥池 ID", "留空使用全部同命途候选")}
      ${catalogue ? cataloguePicker(catalogue) : nothing}
      ${catalogueError ? html`<p class="account-provenance">${catalogueError}</p>` : nothing}
    </details>
    <details class="account-fold">
      <summary>金数分类与资源分配</summary>
      <fieldset class="account-group">
        <legend>计入金数的五星类别</legend>
        <div class="account-checks">${groups.map(key => html`<label>
          <input form=${generationForm} type="checkbox" .checked=${goldCategories.has(key)} ?disabled=${pending}
            @change=${(event: Event) => {
              if (!(event.currentTarget instanceof HTMLInputElement)) return;
              if (event.currentTarget.checked) goldCategories.add(key); else goldCategories.delete(key);
            }} />${groupLabels[key]}
        </label>`)}</div>
      </fieldset>
      <div class="account-fields">
        ${number("resource_concentration", "共用集中度", 0, "any", { max: 1, placeholder: "默认 0" })}
        ${number("gold_concentration", "金数集中度", 0, "any", { max: 1, placeholder: "沿用共用值" })}
        ${number("relic_concentration", "遗器集中度", 0, "any", { max: 1, placeholder: "沿用共用值" })}
        ${number("teammate_bias", "推荐队友关联", 0, "any", { max: 1, placeholder: "默认 0.7" })}
        ${number("light_cone_gold_share", "光锥金数份额", 0, "any", { max: 1, placeholder: "默认 0.25" })}
      </div>
    </details>
    <details class="account-fold">
      <summary>角色练度覆盖与搜索预算</summary>
      <label class="account-field">角色覆盖 JSON
        <textarea form=${generationForm} rows="5" spellcheck="false" placeholder='{"1310": {"eidolons": 2}}'
          .value=${fields.character_overrides ?? ""} ?disabled=${pending} @input=${(event: Event) => {
            if (event.currentTarget instanceof HTMLTextAreaElement) fields.character_overrides = event.currentTarget.value;
          }}></textarea>
      </label>
      ${textField("account_id", "配置 ID", "留空由生成器分配")}
      ${number("max_attempts", "搜索预算", 1, 1, { placeholder: "默认 1000000" })}
    </details>`;

  const render = (): Spread => ({
    title: "添加账号",
    subtitle: "",
    tools: html`<div class="account-workspace-tools">
      <div role="group" aria-label="添加方式">
        <button aria-pressed=${mode === "import"} ?disabled=${pending} @click=${() => changeMode("import")}>文件导入</button>
        ${research ? html`<button aria-pressed=${mode === "generate"} ?disabled=${pending} @click=${() => changeMode("generate")}>随机生成</button>` : nothing}
      </div>
      <button class="account-back" ?disabled=${pending} @click=${close}>← 返回档案</button>
    </div>`,
    left: mode === "import" ? html`<div class="account-workspace">
      <h2>账号文件</h2>
      <p class="account-lead">支持统一账号 JSON。Fribbels v4 导出文件需要配置对应版本的行迹绑定资料。</p>
      <form id=${importForm} @submit=${submit}>
        <label class=${"account-drop" + (source ? " selected-file" : "") + (dragging ? " dragging" : "")}
          @dragover=${(event: DragEvent) => {
            if (pending || !event.dataTransfer?.types.includes("Files")) return;
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
            if (!dragging) { dragging = true; redraw(); }
          }}
          @dragleave=${(event: DragEvent) => {
            if (event.currentTarget instanceof Element && event.relatedTarget instanceof Node &&
              event.currentTarget.contains(event.relatedTarget)) return;
            if (dragging) { dragging = false; redraw(); }
          }}
          @drop=${(event: DragEvent) => {
            event.preventDefault();
            if (event.dataTransfer) return readFile(event.dataTransfer.files);
          }}>
          <input type="file" accept=".json,application/json" aria-label="选择账号文件" ?disabled=${pending}
            @change=${(event: Event) => {
              const input = event.currentTarget;
              if (!(input instanceof HTMLInputElement) || !input.files) return;
              const reading = readFile(input.files);
              input.value = "";
              return reading;
            }} />
          <span class="account-pick">${source ? "更换文件" : "选择文件"}</span>
          <span class="account-file-label" title=${source?.fileName ?? ""}>${source?.fileName ?? "也可将 JSON 拖到这里"}</span>
        </label>
        ${source ? html`<label class="account-field import-name">账号名称
          <input required maxlength="100" .value=${importName} ?disabled=${pending}
            @input=${(event: Event) => {
              if (event.currentTarget instanceof HTMLInputElement) importName = event.currentTarget.value;
            }} />
        </label>` : nothing}
        ${error && !source ? html`<p class="account-error" role="alert">${error}</p>` : nothing}
        ${pending && !source ? html`<p role="status">正在检查文件…</p>` : nothing}
      </form>
    </div>` : html`<div class="account-workspace account-workspace-pane">
      <h2>生成条件</h2>
      <form id=${generationForm} class="account-workspace-pane" @submit=${submit}>
        <div class="scroll-area" role="region" aria-label="生成条件" tabindex="0">
        ${modeForm === "fields" ? html`<div class="account-fields">
          ${number("character_count", "角色数量（必填）", 0, 1, {
            max: catalogue?.characters.length, required: true, placeholder: "输入持有角色数",
          })}
          ${number("gold_count", "目标金数", 0, 0.5, { placeholder: "自动分配" })}
        </div>` : nothing}
        <div class="account-fields">
          <label class="account-field">账号名称
            <input maxlength="100" placeholder="留空自动命名" .value=${name} ?disabled=${pending}
              @input=${(event: Event) => {
                if (event.currentTarget instanceof HTMLInputElement) name = event.currentTarget.value;
              }} />
          </label>
          <label class="account-field">随机种子
            <input inputmode="numeric" pattern="[0-9]+" maxlength="20" placeholder="留空自动生成" .value=${seed}
              ?disabled=${pending} @input=${(event: Event) => {
                if (event.currentTarget instanceof HTMLInputElement) seed = event.currentTarget.value;
              }} />
          </label>
        </div>
        ${modeForm === "fields" ? generationFields() : html`
          <label class="account-field request-editor">生成请求 JSON
            <textarea required rows="15" spellcheck="false" .value=${requestText} ?disabled=${pending}
              @input=${(event: Event) => {
                if (event.currentTarget instanceof HTMLTextAreaElement) requestText = event.currentTarget.value;
              }}></textarea>
          </label>
        `}
        ${error ? html`<p class="account-error" role="alert">${error}</p>` : nothing}
        </div>
        <div class="account-workspace-actions">
          <button class="primary" type="submit" ?disabled=${pending}>${pending ? "生成并保存中…" : "生成并保存"}</button>
        </div>
      </form>
    </div>`,
    right: mode === "import" ? html`<div class="account-workspace account-review account-workspace-pane">
      <div class="account-review-heading">
        <h2>导入检查</h2>
        ${source ? html`<span class="account-status">${source.inspection.report?.warnings.length ? "已校验" : "校验通过"}</span>` : nothing}
      </div>
      ${source ? html`
        <div class="scroll-area" role="region" aria-label="导入检查结果" tabindex="0">
        ${inventorySummary(source.inspection.summary)}
        ${source.inspection.report ? importReportView(source.inspection.report, false) : html`<p class="account-provenance">统一账号格式</p>`}
        ${error ? html`<p class="account-error" role="alert">${error}</p>` : nothing}
        </div>
        <div class="account-workspace-actions">
          <button class="primary" form=${importForm} type="submit" ?disabled=${pending}>${pending ? "保存中…" : "导入并保存"}</button>
        </div>
      ` : html`<p class="account-lead">选中文件后可核对角色、光锥、遗器数量和导出格式。</p>`}
    </div>` : html`<div class="account-workspace account-review">
      <h2>${modeForm === "fields" ? "高级设置" : "请求文件"}</h2>
      ${modeForm === "fields" ? advanced() : html`<p class="account-lead">生成请求沿用 account-generator 的字段。角色数量使用 <code>character_count</code>，角色和装备的细项写入 <code>character_overrides</code>。</p>`}
      <div class="account-workspace-actions">
        <label class="account-request-file">载入请求 JSON
          <input type="file" accept=".json,application/json" aria-label="载入请求 JSON" ?disabled=${pending}
            @change=${(event: Event) => {
              const input = event.currentTarget;
              if (!(input instanceof HTMLInputElement)) return;
              const file = input.files?.item(0);
              input.value = "";
              if (file) return readRequest(file);
            }} />
        </label>
        ${modeForm === "fields" ? html`<button type="button" ?disabled=${pending} @click=${() => {
          try {
            requestText = JSON.stringify(requestFromFields(), null, 2);
            modeForm = "json";
            error = "";
          } catch (cause) {
            error = cause instanceof Error ? cause.message : String(cause);
          }
          redraw();
        }}>编辑请求 JSON</button>` : html`<button type="button" ?disabled=${pending} @click=${() => {
          modeForm = "fields";
          error = "";
          redraw();
        }}>返回表单草稿</button>`}
      </div>
    </div>`,
  });
  return { render, loadCatalogue, changeMode };
}
