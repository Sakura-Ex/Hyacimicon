import { html, nothing } from "lit";
import type { Account } from "./contracts.ts";
import { auraList, itemPicture, portrait } from "./match-board.ts";
import { crystalNames, mechanismNames, type MatchCatalogue } from "./match-catalogue.ts";
import { entryList, type MatchControls } from "./match-controls.ts";
import type { ChoiceOption, CreateMatchInput, MatchPhase, MatchSnapshot, NodeSettlement, TraitChoice, VisibleMechanism } from "./match-session-contract.ts";
import type { MatchInsights } from "./match-port.ts";

export const phaseNames = {
  investmentEnvironment: "投资环境待选", preparation: "备战中", investmentStrategy: "投资策略待选",
  supply: "补给待选", spotlight: "主角光环待选", teaBreak: "幕后茶歇", teaBreakReward: "茶歇领奖",
  battle: "战斗中", battleAway: "战斗暂离", battleUnavailable: "战斗暂不可用", finished: "对局结束",
} satisfies Record<MatchPhase["kind"], string>;

export function setup(account: Account, busy: boolean, demoVersion: string | undefined, create: (input: CreateMatchInput) => void) {
  const left = html`<section class="session-intel"><h2>${account.name}</h2>
    <p>${account.characters.length} 名角色 · ${account.source}</p>
    ${entryList(account.characters.map(unit => ({ name: unit.name,
      detail: unit.level === null ? undefined : `Lv.${unit.level}${unit.eidolons === null ? "" : ` · ${unit.eidolons} 星魂`}` })), "暂无角色档案")}
    <p>开始后先查看对手和负面词条，再选择投资环境。</p></section>`;
  const right = html`<form class="match-setup" @submit=${(event: SubmitEvent) => {
    event.preventDefault();
    if (!(event.currentTarget instanceof HTMLFormElement)) return;
    const form = new FormData(event.currentTarget);
    const seed = form.get("seed");
    const rulesVersion = form.get("rulesVersion");
    if (typeof rulesVersion !== "string") return;
    create({ accountId: account.id, rulesVersion: rulesVersion.trim(),
      enemyDifficulty: Number(form.get("enemyDifficulty")), affixCount: Number(form.get("affixCount")),
      randomness: typeof seed === "string" && seed.trim() ? { kind: "seeded", seed: seed.trim() } : { kind: "automatic" } });
  }}>
    <div class="match-content scroll-area">
      <h2>开局配置</h2>
      <fieldset ?disabled=${busy}>
        <label>规则版本 <input name="rulesVersion" required .value=${demoVersion ?? ""} ?readonly=${!!demoVersion}></label>
        <label>敌人难度 <input name="enemyDifficulty" type="number" min="1" max=${demoVersion ? 10 : nothing} value="8" required></label>
        <label>负面词条数量 <input name="affixCount" type="number" min="0" max=${demoVersion ? 3 : nothing} value="2" required></label>
        <label>随机种子 <input name="seed" placeholder="留空自动随机"></label>
      </fieldset>
    </div>
    <footer class="match-actions"><button type="submit" class="primary" ?disabled=${busy}>开始新对局</button></footer>
  </form>`;
  return { left, right };
}

export function settlement(s: NodeSettlement) {
  return html`<details class="settlement">
    <summary>${s.nodeId} 结算 · 血量 ${s.healthBefore} → ${s.healthAfter} · 金币 +${s.baseGold + s.interestGold + s.streakGold}</summary>
    <dl>
      <div><dt>血量</dt><dd>失血 ${s.healthLost} · 固定回复 +${s.fixedRecovery} · 其他变化 ${s.otherHealthDelta}</dd></div>
      <div><dt>金币</dt><dd>基础 ${s.baseGold} · 利息 ${s.interestGold} · 连胜 ${s.streakGold}</dd></div>
      <div><dt>火热连胜</dt><dd>${s.hotStreakAfter}</dd></div>
      <div><dt>经验</dt><dd>+${s.experienceGained}</dd></div>
      <div><dt>晶矿</dt><dd>${s.crystalsGranted.length ? s.crystalsGranted.map(crystal =>
        `${crystalNames[crystal.quality]}色 ×${crystal.count}`).join(" · ") : "无"}</dd></div>
    </dl>
  </details>`;
}

export function traitChoice(mechanism: VisibleMechanism, choice: TraitChoice, controls: MatchControls, draft?: string | null) {
  const { button, find } = controls;
  const key = (a: { mechanism: string; choiceId: string }) => a.mechanism === mechanism.kind && a.choiceId === choice.choiceId;
  return html`<section class="session-phase">
    <h2>${mechanismNames[mechanism.kind]}</h2>
    <div class="phase-cards">${choice.options.map(option => {
      const entry = draft === undefined
        ? find("chooseMechanism", a => a.kind === "chooseMechanism" && key(a) && a.optionId === option.optionId)
        : find("selectTraitOption", a => a.kind === "selectTraitOption" && key(a) && a.optionId === option.optionId);
      return html`<article><h3>${option.name}</h3><p>${option.detail}</p>
        ${choice.selectedOptionId === option.optionId ? html`<small>当前生效</small>` : nothing}
        ${draft === option.optionId ? html`<small>待确认</small>` : nothing}
        ${button(entry, draft === undefined ? "确认此项" : "选为待确认", false, draft === undefined ? undefined : draft === option.optionId)}
      </article>`;
    })}</div>
    ${draft === undefined ? nothing : button(find("confirmTraitChoice", a => a.kind === "confirmTraitChoice" && key(a)), "确认羁绊选择", true)}
  </section>`;
}

const qualities = { silver: "银", gold: "金", prismatic: "彩" };

export function phasePanel(s: MatchSnapshot, c: MatchCatalogue, controls: MatchControls, insights: MatchInsights | undefined,
  auraTarget: string | undefined, selectAura: (id: string) => void) {
  const { phase, preparation: p, companion } = s;
  const { find, button } = controls;
  const optionAction = (kind: Parameters<typeof find>[0], optionId: string) =>
    find(kind, a => "optionId" in a && a.optionId === optionId && (!("pieceId" in a) || a.pieceId === auraTarget));
  function card(option: ChoiceOption, select: Parameters<typeof find>[0], refresh = false) {
    const entry = find("refreshChoice", a => a.kind === "refreshChoice" && a.optionId === option.optionId);
    const quality = phase.kind === "investmentStrategy" ? phase.options.find(o => o.optionId === option.optionId)?.quality : undefined;
    return html`<article data-quality=${quality ?? nothing}>
      ${quality ? html`<small>${qualities[quality]}色</small>` : nothing}<h3>${option.name}</h3><p>${option.detail}</p>
      <div>${button(optionAction(select, option.optionId), "选择", true)}${refresh ? button(entry, "刷新") : nothing}</div>
    </article>`;
  }
  switch (phase.kind) {
    case "investmentEnvironment": return html`<section class="session-phase">
      <h2>选择投资环境</h2><div class="phase-cards">${phase.options.map(option => card(option, "chooseInvestmentEnvironment", true))}</div>
    </section>`;
    case "investmentStrategy": return html`<section class="session-phase">
      <h2>选择投资策略</h2><p>确认后正式进入 ${s.position?.nodeId} 并领取入节点收益。当前金币 ${p?.resources.gold ?? "—"}。</p>
      <div class="phase-cards">${phase.options.map(option => card(option, "chooseInvestmentStrategy", true))}</div>
    </section>`;
    case "supply": return html`<section class="session-phase">
      <div class="phase-title"><h2>领取补给</h2>${button(find("refreshSupply"), "刷新全部")}</div>
      <div class="phase-cards supply-cards">${phase.options.map(option => {
        const unit = c.unit(option.unitId);
        return html`<article><span class="offer-portrait">${portrait(unit)}</span><h3>${unit.name}</h3>
          <ul>${option.itemDefinitionIds.map(id => {
            const item = c.item(id);
            return html`<li>${itemPicture(item)} <span>${item.name}</span></li>`;
          })}</ul>${button(optionAction("takeSupply", option.optionId), "领取", true)}</article>`;
      })}</div>
    </section>`;
    case "spotlight": return html`<section class="session-phase">
      <h2>选择主角光环</h2>
      <p>已刷新 ${phase.refreshCount} 次 · 优惠上限 ${phase.discountedRefreshLimit} 次 · 本次 ${phase.refreshPrice} 金币</p>
      <label>目标角色 <select .value=${auraTarget ?? ""} ?disabled=${!controls.find("chooseSpotlight", a => a.kind === "chooseSpotlight" && a.pieceId === auraTarget) &&
        !controls.actions.some(a => a.action.kind === "chooseSpotlight")} @change=${(event: Event) => {
        if (event.target instanceof HTMLSelectElement) selectAura(event.target.value);
      }}>${phase.eligiblePieceIds.map(id => {
        const piece = p?.board.pieces.find(piece => piece.pieceId === id);
        return html`<option value=${id} ?selected=${id === auraTarget}>
          ${c.pieceName(id)}${piece ? ` · ${piece.stars} 星 · 格位 ${piece.slot.index + 1}` : ""}
        </option>`;
      })}</select></label>
      <div class="phase-cards">${phase.options.map(option => card(option, "chooseSpotlight", true))}</div>
    </section>`;
    case "teaBreak": return html`<section class="session-phase">
      <h2>幕后茶歇</h2>${companion ? entryList([companion]) : nothing}
      <div class="phase-cards">${phase.options.map(option => card(option, "chooseTeaBreak"))}</div>
    </section>`;
    case "teaBreakReward": return html`<section class="session-phase"><h2>茶歇领奖</h2>
      <p>${insights?.selectedTea?.name ?? phase.selectedOptionId}</p>
      <p>领取原奖励关晶矿、基础金币、利息和火热连胜奖励。经验 +0。</p>
    </section>`;
    case "battle": return html`<section class="session-phase"><h2>战斗进行中</h2>
      <p>节点 ${s.position?.nodeId} · ${phase.attemptId}</p>${entryList(p?.revealedEnemies ?? [], "当前敌人待揭示")}
      <p>重开沿用本次尝试的随机序列。</p>${button(find("restartBattle"), "重开战斗")}
    </section>`;
    case "battleAway": return html`<section class="session-phase"><h2>战斗暂离</h2>
      <p>可在左页调整站位。重新进入后从头开战，使用新的随机序列。</p></section>`;
    case "battleUnavailable": return html`<section class="session-phase"><h2>战斗暂不可用</h2>
      <p role="alert">${phase.message}</p><p>本场尚未结算，仍处于当前节点。</p>
      ${button(find("leaveBattle"), "暂离战斗")}
    </section>`;
    case "finished": return html`<section class="session-phase">
      <h2>${phase.reason === "cleared" ? "本局通关" : "整局血量耗尽"}</h2>
      <p>最终节点 ${s.position?.nodeId} · 整局血量 ${p?.resources.health ?? "—"}</p>
      ${p ? html`<h3>历次投资</h3>${entryList([...(s.environment ? [s.environment] : []), ...p.selectedInvestments])}
        <h3>主角光环</h3>${auraList(p, c)}` : nothing}
    </section>`;
    case "preparation": return nothing;
  }
}
