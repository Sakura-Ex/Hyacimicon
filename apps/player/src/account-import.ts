import { html, nothing } from "lit";
import type { Account, PlayerPort } from "./contracts";
import type { Run, Spread } from "./folio";
import "./account-import.css";
import archiveArt from "./account-archive.webp";

export function accountImportView(
  port: Pick<PlayerPort, "importAccount" | "generateAccount">,
  redraw: () => void,
  run: Run,
  select: (account: Account) => void,
  back: () => void,
) {
  let mode: "file" | "seed" = "file";
  let file: File | undefined;
  let seed = "1";
  let error = "";
  let pending = false;

  function chooseFile(files: FileList) {
    if (!files.length) return;
    const candidate = files.item(0);
    error = "";
    if (files.length !== 1 || !candidate || !/\.json$/i.test(candidate.name)) {
      file = undefined;
      error = "请选择一个 .json 文件。";
    } else {
      file = candidate;
    }
    redraw();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const source = file;
    const importFile = source && (async () => port.importAccount(source.name, await source.text()));
    const submitAccount = mode === "file" ? importFile : () => port.generateAccount(Number(seed));
    if (!submitAccount) return;
    return run(async () => {
      pending = true;
      error = "";
      redraw();
      try {
        select(await submitAccount());
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
      } finally {
        pending = false;
      }
    });
  }

  return (): Spread => ({
    title: "添加账号",
    subtitle: "",
    tools: html`<button class="import-back" ?disabled=${pending} @click=${back}><span aria-hidden="true">←</span> 角色档案</button>`,
    layout: "account-import",
    left: html`<div class="import-intro">
      <h2>添加方式</h2>
      <div class="import-modes" role="group" aria-label="添加方式">
        ${(["file", "seed"] as const).map((next) => html`<button
          type="button"
          aria-pressed=${mode === next}
          ?disabled=${pending}
          @click=${() => {
            if (mode === next) return;
            mode = next;
            error = "";
            redraw();
          }}
        >
          <span>
            <strong>${mode === next ? html`<span class="import-current-mark" aria-hidden="true"></span>` : nothing}${next === "file" ? "文件导入" : "随机生成"}</strong>
            <small>${next === "file" ? "带入已有角色、行迹、光锥和遗器" : "用种子创建可复现的练习账号"}</small>
          </span>
        </button>`)}
      </div>
    </div>`,
    right: html`<div class="import-workspace">
      <form class="import-form" @submit=${submit}>
        ${mode === "file" ? html`
          <h1>导入已有账号</h1>
          <div class="import-envelope">
          <label class="import-drop"
            @dragover=${(event: DragEvent) => event.preventDefault()}
            @drop=${(event: DragEvent) => {
              event.preventDefault();
              if (!pending && event.dataTransfer) chooseFile(event.dataTransfer.files);
            }}
          >
            <input aria-label=${file ? "更换 JSON 文件" : "选择 JSON 文件"} type="file"
              accept=".json,application/json" ?disabled=${pending}
              @change=${(event: Event) => {
                const input = event.currentTarget;
                if (!(input instanceof HTMLInputElement) || !input.files) return;
                chooseFile(input.files);
                input.value = "";
              }} />
            <img class="import-art" src=${archiveArt} alt="" />
            <span class=${file ? "import-drop-action import-drop-selected" : "import-drop-action"}>
              ${file ? html`<span class="import-filename" role="status" title=${file.name}>${file.name}</span>` : nothing}
              ${file ? nothing : html`<span class="import-source">使用 reliquary-archiver 导出的 Fribbels JSON。</span>`}
              <span class="import-pick">${file ? "更换文件" : "选择 JSON 文件"}</span>
            </span>
            ${file ? nothing : html`<small class="import-drop-hint">或将 .json 文件拖入袋口</small>`}
          </label>
          ${file ? html`<div class="import-submit import-file-submit">
            <button class="primary" type="submit" ?disabled=${pending}>${pending ? "处理中…" : "确认导入"}</button>
          </div>` : nothing}
          </div>
        ` : html`
          <h1>生成练习账号</h1>
          <p>通过种子复现同一份账号配置。</p>
          <label class="import-seed">随机种子
            <input type="number" required min="0" max="4294967295" step="1"
              .value=${seed} ?disabled=${pending}
              @input=${(event: Event) => {
                if (event.currentTarget instanceof HTMLInputElement) seed = event.currentTarget.value;
              }} />
            <small>0 到 4,294,967,295 的整数</small>
          </label>
        `}
        ${error ? html`<p class="import-error" role="alert">${error}</p>` : nothing}
        ${mode === "seed" ? html`<div class="import-submit">
          <button class="primary" type="submit" ?disabled=${pending}>
            ${pending ? "处理中…" : "生成账号"}
          </button>
        </div>` : nothing}
      </form>
    </div>`,
  });
}
