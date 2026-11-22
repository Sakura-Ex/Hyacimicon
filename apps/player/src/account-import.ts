import { html, nothing } from "lit";
import { ref } from "lit/directives/ref.js";
import type { Account, PlayerPort } from "./contracts";
import { icon, type Run } from "./folio";
import "./account-import.css";

export function accountImportView(
  port: Pick<PlayerPort, "importAccount" | "generateAccount">,
  redraw: () => void,
  run: Run,
  select: (account: Account) => void,
  dismiss: () => void,
) {
  let mode: "file" | "seed" = "file";
  let file: File | undefined;
  let seed = "1";
  let error = "";
  let pending = false;
  let dragging = false;

  function close() {
    if (!pending) dismiss();
  }

  function focusInput(element: Element | undefined) {
    if (element instanceof HTMLInputElement) element.focus({ preventScroll: true });
  }

  function chooseFile(files: FileList) {
    if (!files.length) return;
    const candidate = files.item(0);
    if (files.length !== 1 || !candidate || !/\.json$/i.test(candidate.name)) {
      error = "请选择一个 .json 文件。";
    } else {
      file = candidate;
      error = "";
    }
  }

  function switchMode() {
    mode = mode === "file" ? "seed" : "file";
    error = "";
    redraw();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const source = file;
    const importFile = source && (async () => port.importAccount(source.name, await source.text()));
    const submitAccount = mode === "file" ? importFile : () => port.generateAccount(Number(seed));
    if (!submitAccount || pending) return;
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

  const render = () => html`<form
    id="account-note"
    class="account-note ${dragging ? "dragging" : ""}"
    aria-labelledby="account-note-title"
    aria-busy=${pending}
    @submit=${submit}
    @dragover=${(event: DragEvent) => {
      if (mode !== "file" || pending || !event.dataTransfer?.types.includes("Files")) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
      if (!dragging) {
        dragging = true;
        redraw();
      }
    }}
    @dragleave=${(event: DragEvent) => {
      if (!dragging) return;
      if (event.currentTarget instanceof HTMLElement &&
          event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return;
      dragging = false;
      redraw();
    }}
    @drop=${(event: DragEvent) => {
      event.preventDefault();
      if (mode !== "file" || pending || !event.dataTransfer) return;
      dragging = false;
      chooseFile(event.dataTransfer.files);
      redraw();
    }}
  >
    <div class="note-heading">
      <h2 id="account-note-title">${mode === "file" ? "导入账号" : "生成练习账号"}</h2>
      <button class="note-close" type="button" aria-label="关闭便笺" ?disabled=${pending} @click=${close}>${icon("close")}</button>
    </div>
    ${mode === "file" ? html`
      <div class="note-file">
        ${file ? html`<p>已选择文件</p>` : nothing}
        <span class="note-filename" role="status" title=${file?.name ?? ""}>${file?.name ?? ""}</span>
        <div class="note-actions">
          <label class=${file ? "note-picker" : "note-picker primary"}>
            ${file ? "更换文件" : "选择文件"}
            <input ${ref(focusInput)} type="file" accept=".json,application/json"
              aria-label=${file ? "更换文件" : "选择文件"} ?disabled=${pending}
              @change=${(event: Event) => {
                const input = event.currentTarget;
                if (!(input instanceof HTMLInputElement) || !input.files) return;
                chooseFile(input.files);
                input.value = "";
                redraw();
              }} />
          </label>
          ${file ? html`<button class="primary" type="submit" ?disabled=${pending}>${pending ? "导入中…" : "确认导入"}</button>` : nothing}
          ${file ? nothing : html`<small>或拖入 Fribbels JSON</small>`}
        </div>
      </div>
    ` : html`
      <label class="note-seed">随机种子
        <input ${ref(focusInput)} type="number" min="0" max="4294967295" step="1" required
          .value=${seed} ?disabled=${pending}
          @input=${(event: Event) => {
            if (event.currentTarget instanceof HTMLInputElement) seed = event.currentTarget.value;
          }} />
      </label>
      <p>同一种子可复现同一份练习账号。</p>
      <div class="note-actions"><button class="primary" type="submit" ?disabled=${pending}>${pending ? "生成中…" : "生成账号"}</button></div>
    `}
    ${error ? html`<p class="note-error" role="alert">${error}</p>` : nothing}
    <div class="note-footer">
      <button type="button" ?disabled=${pending} @click=${switchMode}>${mode === "file" ? "随机生成练习账号" : "返回文件导入"}</button>
    </div>
  </form>`;
  return { render, close };
}
