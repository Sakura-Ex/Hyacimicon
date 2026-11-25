import { html, nothing } from "lit";
import { previewGroups } from "./developer-preview";
import "./settings.css";

const developerModeKey = "hyacimicon.developerMode";
type Target = (typeof previewGroups)[number]["targets"][number];

export function playerSettings(redraw: () => void, openPreview: (target: Target) => void, leavePreview: () => void) {
  let developerMode = false;
  let filter = "";
  let storageMessage = "";
  try {
    developerMode = localStorage.getItem(developerModeKey) === "true";
  } catch {
    storageMessage = "此设备无法读取设置。";
  }
  const dialog = () => document.querySelector<HTMLDialogElement>("#player-settings");
  function close() { dialog()?.close(); }
  function open() {
    const surface = dialog();
    if (surface && !surface.open) {
      filter = "";
      redraw();
      surface.showModal();
      surface.querySelector<HTMLInputElement>(".settings-search input")?.focus();
    }
  }
  function setDeveloperMode(value: boolean) {
    developerMode = value;
    if (!value) leavePreview();
    try {
      localStorage.setItem(developerModeKey, String(value));
      storageMessage = "";
    } catch {
      storageMessage = "设置已应用于本次会话，设备暂时无法保存。";
    }
    redraw();
    if (value) dialog()?.querySelector<HTMLInputElement>(".settings-search input")?.focus();
  }
  function launch(target: Target) {
    close();
    openPreview(target);
  }
  function view() {
    const query = filter.trim().toLocaleLowerCase();
    const groups = previewGroups.map(group => ({
      ...group, targets: group.targets.filter(target => (group.title + target.label).toLocaleLowerCase().includes(query)),
    })).filter(group => group.targets.length);
    return html`<dialog id="player-settings" class="player-settings" aria-labelledby="settings-title">
      <div class="settings-heading">
        <h2 id="settings-title">设置</h2>
        <button type="button" aria-label="关闭设置" @click=${close}>关闭</button>
      </div>
      <div class="settings-scroll">
        <label class="settings-toggle">
          <input type="checkbox" role="switch" .checked=${developerMode} @change=${(event: Event) => {
            if (event.currentTarget instanceof HTMLInputElement) setDeveloperMode(event.currentTarget.checked);
          }}>
          <span><strong>开发者模式</strong><small>打开界面样例，检查布局和交互。</small></span>
        </label>
        ${storageMessage ? html`<p role="status">${storageMessage}</p>` : nothing}
        ${developerMode ? html`<section class="settings-previews" aria-labelledby="preview-title">
          <h3 id="preview-title">开发预览</h3>
          <label class="settings-search"><span>搜索界面</span>
            <input type="search" .value=${filter} @input=${(event: Event) => {
              if (event.currentTarget instanceof HTMLInputElement) {
                filter = event.currentTarget.value;
                redraw();
              }
            }} @keydown=${(event: KeyboardEvent) => {
              if (event.key !== "Enter" || event.isComposing || groups.length !== 1 || groups[0].targets.length !== 1) return;
              event.preventDefault();
              launch(groups[0].targets[0]);
            }}>
          </label>
          ${groups.map(group => html`<div class="settings-preview-group">
            <h4>${group.title}</h4>
            <div>${group.targets.map(target => html`<button type="button" @click=${() => launch(target)}>${target.label}</button>`)}</div>
          </div>`)}
          ${groups.length ? nothing : html`<p role="status">没有匹配的界面。</p>`}
        </section>` : nothing}
      </div>
    </dialog>`;
  }
  return { open, view, get developerMode() { return developerMode; } };
}
