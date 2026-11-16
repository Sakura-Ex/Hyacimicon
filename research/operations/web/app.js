const appState = {
  meta: null,
  session: null,
  busy: false,
  drawer: null,
};

const byId = (id) => document.getElementById(id);

const escapeHtml = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");

const nodeGlyphs = {
  battle: "⚔",
  encounter: "?",
  boss: "♛",
  reward: "✦",
};

const nodeLabels = {
  battle: "战斗",
  encounter: "遭遇",
  boss: "首领",
  reward: "奖励",
};

async function request(url, options = {}) {
  const response = await fetch(url, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(payload.error || `请求失败：${response.status}`);
  }
  return payload;
}

function showError(error) {
  const toast = byId("toast");
  toast.textContent = error instanceof Error ? error.message : String(error);
  toast.hidden = false;
  window.clearTimeout(showError.timer);
  showError.timer = window.setTimeout(() => {
    toast.hidden = true;
  }, 4300);
}

function setBusy(busy) {
  appState.busy = busy;
  document.body.classList.toggle("is-busy", busy);
}

function starText(star) {
  return "★".repeat(Math.max(1, Number(star)));
}

function portraitMarkup(unit, className = "") {
  if (unit.portrait_path) {
    return `<img class="${className}" src="${escapeHtml(unit.portrait_path)}" alt="">`;
  }
  return `<div class="${className} trait-fallback">${escapeHtml(unit.name.slice(0, 1))}</div>`;
}

function populateMeta(meta) {
  byId("game-version").textContent = `数据 ${meta.game_version} · S2`;
  byId("snapshot-id").textContent = meta.snapshot_id || "内置规则";
  byId("start-snapshot").textContent = meta.snapshot_id || "内置规则";
  byId("start-counts").textContent =
    `${meta.character_count} 名角色 · ${meta.trait_count} 个羁绊 · 20 个节点`;
  byId("meta-status").textContent =
    `${meta.validation}。角色与羁绊来自私有数据仓库；投资数值仍按当前规则包运行。`;

  byId("difficulty").innerHTML = meta.difficulties
    .map(
      (difficulty) =>
        `<option value="${escapeHtml(difficulty.id)}">${escapeHtml(difficulty.name)} · ${difficulty.affix_count} 词缀</option>`,
    )
    .join("");
  byId("environment").innerHTML =
    '<option value="">随机环境</option>' +
    meta.environments
      .map(
        (environment) =>
          `<option value="${escapeHtml(environment.id)}">${escapeHtml(environment.name)}</option>`,
      )
      .join("");
}

async function startRun(event) {
  event.preventDefault();
  if (appState.busy) {
    return;
  }
  setBusy(true);
  try {
    appState.session = await request("/api/sessions", {
      method: "POST",
      body: JSON.stringify({
        difficulty_id: byId("difficulty").value,
        environment_id: byId("environment").value || null,
        seed: Number(byId("seed").value),
        battle_progress: Number(byId("start-progress").value),
      }),
    });
    byId("start-screen").hidden = true;
    render();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

async function runAction(action) {
  if (!appState.session || appState.busy) {
    return;
  }
  setBusy(true);
  try {
    appState.session = await request(
      `/api/sessions/${appState.session.session_id}/actions`,
      {
        method: "POST",
        body: JSON.stringify(action),
      },
    );
    render();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

function renderRoute(session) {
  const results = new Map(
    session.results.map((result) => [result.node_index, result]),
  );
  byId("node-track").innerHTML = session.node_template
    .map((node) => {
      const result = results.get(node.index);
      const status = result ? (result.won ? "won" : "lost") : "";
      const current =
        node.index === session.node.index && !session.finished ? "current" : "";
      return `
        <div class="route-node ${status} ${current}" title="${escapeHtml(node.id)} · ${escapeHtml(nodeLabels[node.type])}">
          <span class="node-glyph">${nodeGlyphs[node.type] || "•"}</span>
          <small>${escapeHtml(node.id)}</small>
        </div>
      `;
    })
    .join("");
}

function allocateUnits(activeUnits) {
  const front = activeUnits.filter((unit) => unit.position === "front");
  const back = activeUnits.filter((unit) => unit.position === "back");
  const flex = activeUnits.filter((unit) => unit.position === "flex");
  for (const unit of flex) {
    if (front.length <= back.length) {
      front.push(unit);
    } else {
      back.push(unit);
    }
  }
  return { front, back };
}

function unitCard(unit) {
  const cost = unit.costs?.[0] ?? unit.cost;
  return `
    <article class="unit-slot filled" title="${escapeHtml(unit.description || "")}">
      ${portraitMarkup(unit)}
      <span class="cost-badge">${cost}</span>
      <span class="star-badge">${starText(unit.star)}</span>
      <div class="unit-info">
        <strong>${escapeHtml(unit.name)}</strong>
        <small>${escapeHtml(unit.position === "front" ? "前台" : unit.position === "back" ? "后台" : "前后台")}</small>
      </div>
    </article>
  `;
}

function renderUnits(session) {
  const { front, back } = allocateUnits(session.active_units);
  const expandSlots = (units, minimum) => {
    const cards = units.map(unitCard);
    while (cards.length < minimum) {
      cards.push('<div class="unit-slot empty"></div>');
    }
    return cards.join("");
  };
  byId("front-units").innerHTML = expandSlots(front, 5);
  byId("back-units").innerHTML = expandSlots(back, 6);
  byId("active-capacity").textContent =
    `${session.active_units.length}/${session.resources.level}`;
}

function renderTraits(session) {
  const traits = session.active_traits;
  byId("trait-count").textContent = traits.length;
  byId("active-traits").innerHTML =
    traits
      .map((trait) => {
        const icon = trait.icon_path
          ? `<img src="${escapeHtml(trait.icon_path)}" alt="">`
          : '<span class="trait-fallback">◆</span>';
        const next =
          trait.next_tier === null
            ? "已满"
            : `${trait.count}/${trait.next_tier}`;
        return `
          <article class="trait-entry ${trait.active_tier !== null ? "active" : ""}">
            ${icon}
            <div>
              <strong>${escapeHtml(trait.name)}</strong>
              <small>${escapeHtml(trait.group)} · ${next}</small>
            </div>
          </article>
        `;
      })
      .join("") || '<p class="trait-empty">当前阵容尚未形成羁绊</p>';
}

function renderShop(session) {
  byId("shop").innerHTML = session.shop
    .map((offer, index) => {
      if (offer.empty) {
        return '<article class="shop-card empty"><span>已售出</span></article>';
      }
      const enabled =
        session.capabilities.buy_offer && offer.affordable ? "" : "disabled";
      return `
        <article class="shop-card">
          ${portraitMarkup(offer)}
          <div class="shop-card-body">
            <strong>${escapeHtml(offer.name)}</strong>
            <small>${escapeHtml(offer.roles.join(" / ") || "角色")}</small>
            <small class="price">${offer.cost} 金币 · ${escapeHtml(offer.traits.join(" / "))}</small>
            <button type="button" data-action="buy" data-index="${index}" ${enabled}>购买</button>
          </div>
        </article>
      `;
    })
    .join("");
  byId("shop-gold").textContent = `${session.resources.gold} 金币`;

  const lock = byId("toggle-lock");
  lock.textContent = session.shop_locked ? "开" : "锁";
  lock.title = session.shop_locked ? "解除锁定" : "锁定商店";
  lock.disabled = !session.capabilities.lock_shop;
  byId("reroll-shop").disabled = !session.capabilities.reroll_shop;
  byId("buy-xp").disabled = !session.capabilities.buy_xp;
  byId("advance-node").disabled = !session.capabilities.advance_node;
  byId("free-rerolls").textContent = session.resources.free_rerolls;
}

function renderResources(session) {
  byId("stage-number").textContent = session.node.id || session.node.number;
  byId("health").textContent = `${session.resources.health}/${session.resources.max_health}`;
  byId("level").textContent = session.resources.level;
  byId("xp-line").textContent =
    session.resources.xp_to_next_level === null
      ? `${session.resources.xp} · 已满级`
      : `${session.resources.xp}/${session.resources.xp_to_next_level}`;
  byId("run-environment").textContent = session.environment.name;
  byId("run-difficulty").textContent = session.difficulty.name;
  byId("run-plane").textContent = `位面 ${session.node.plane}`;
  byId("progress-value").textContent =
    `${session.battle.progress_percent}%`;
  byId("battle-progress").value = session.battle.progress_percent;
  byId("apply-progress").disabled = !session.capabilities.set_battle_progress;
  byId("advance-label").textContent =
    session.phase === "investment" ? "请先选择投资" : "结算当前节点";
}

function renderInvestmentModal(session) {
  const modal = byId("investment-modal");
  const offers = session.investments.offers;
  modal.hidden = offers.length === 0;
  byId("investment-options").innerHTML = offers
    .map(
      (investment, index) => `
        <article class="investment-option">
          <div>
            <span class="rarity">${escapeHtml(investment.rarity)}</span>
            <h3>${escapeHtml(investment.name)}</h3>
            <p>${escapeHtml(investment.description)}</p>
          </div>
          <button type="button" data-action="investment" data-index="${index}">
            选择
          </button>
        </article>
      `,
    )
    .join("");
}

function renderFinish(session) {
  const banner = byId("finish-banner");
  banner.hidden = !session.finished;
  if (session.finished) {
    banner.textContent = session.won
      ? "整局胜利"
      : `对局结束 · 抵达 ${session.node.id}`;
  }
}

function renderDrawer(session) {
  if (!appState.drawer) {
    byId("drawer").hidden = true;
    return;
  }
  byId("drawer").hidden = false;
  const titles = {
    roster: ["阵容与备战席", "全部角色"],
    investments: ["投资与环境", "当前加成"],
    log: ["动作与节点", "运行记录"],
    debug: ["规则检查", "内部状态"],
  };
  const [kicker, title] = titles[appState.drawer];
  byId("drawer-kicker").textContent = kicker;
  byId("drawer-title").textContent = title;

  if (appState.drawer === "roster") {
    byId("drawer-body").innerHTML =
      `<div class="roster-list">${session.roster
        .map(
          (unit) => `
            <article class="roster-item">
              ${portraitMarkup(unit)}
              <div>
                <strong>${escapeHtml(unit.name)} · ${starText(unit.star)}</strong>
                <small>${unit.cost}费 · ${escapeHtml(unit.position)} · ${escapeHtml(unit.traits.join(" / "))}</small>
              </div>
              <button type="button" data-action="sell" data-unit-id="${escapeHtml(unit.unit_id)}" data-star="${unit.star}" ${session.capabilities.sell_unit ? "" : "disabled"}>
                出售
              </button>
            </article>
          `,
        )
        .join("")}</div>`;
    return;
  }

  if (appState.drawer === "investments") {
    const selected = session.investments.selected
      .map(
        (investment) => `
          <article class="info-card">
            <span>◆</span>
            <div>
              <strong>${escapeHtml(investment.name)}</strong>
              <small>${escapeHtml(investment.rarity)}</small>
              <p>${escapeHtml(investment.description)}</p>
            </div>
          </article>
        `,
      )
      .join("");
    const affixes = session.affixes
      .map(
        (affix) => `
          <article class="info-card">
            <span>!</span>
            <div>
              <strong>${escapeHtml(affix.name)}</strong>
              <p>${escapeHtml(affix.description)}</p>
            </div>
          </article>
        `,
      )
      .join("");
    byId("drawer-body").innerHTML =
      `<h3>已选投资</h3><div class="selected-list">${selected || '<p class="muted">暂无</p>'}</div>
       <h3>敌人词缀</h3><div class="selected-list">${affixes || '<p class="muted">暂无</p>'}</div>`;
    return;
  }

  if (appState.drawer === "log") {
    byId("drawer-body").innerHTML = `
      <ol class="record-list">
        ${[...session.events]
          .reverse()
          .map(
            (event) => `
              <li>
                <span>#${event.sequence}</span>
                <div>${escapeHtml(event.message)}</div>
              </li>
            `,
          )
          .join("")}
      </ol>
      <h3>节点结果</h3>
      <div class="table-wrap">
        <table>
          <thead><tr><th>节点</th><th>进度</th><th>生命</th><th>金币</th></tr></thead>
          <tbody>
            ${session.results
              .map(
                (result) => `
                  <tr>
                    <td>${result.node_index + 1} · ${escapeHtml(result.node_type)}</td>
                    <td class="${result.won ? "win" : "loss"}">${result.battle_progress}%</td>
                    <td>${result.health_delta > 0 ? "+" : ""}${result.health_delta}</td>
                    <td>${result.gold_delta > 0 ? "+" : ""}${result.gold_delta}</td>
                  </tr>
                `,
              )
              .join("") || '<tr><td colspan="4" class="muted">尚未结算节点</td></tr>'}
          </tbody>
        </table>
      </div>`;
    return;
  }

  byId("drawer-body").innerHTML = `
    <div class="metric-grid">
      <div><small>阵容强度</small><strong>${session.debug.team_power.toFixed(2)}</strong></div>
      <div><small>节点强度</small><strong>${session.debug.node_enemy_rating === null ? "—" : session.debug.node_enemy_rating.toFixed(2)}</strong></div>
      <div><small>随机种子</small><strong>${session.seed}</strong></div>
    </div>
    <p class="muted">数据快照：${escapeHtml(session.snapshot_id || "内置规则")}</p>
    <h3>剩余牌池</h3>
    <div class="table-wrap">
      <table>
        <thead><tr><th>角色</th><th>费用</th><th>剩余</th></tr></thead>
        <tbody>
          ${session.debug.pool
            .map(
              (unit) => `
                <tr>
                  <td>${escapeHtml(unit.name)}</td>
                  <td>${unit.cost}</td>
                  <td>${unit.remaining}</td>
                </tr>
              `,
            )
            .join("")}
        </tbody>
      </table>
    </div>`;
}

function render() {
  const session = appState.session;
  if (!session) {
    byId("start-screen").hidden = false;
    return;
  }
  byId("start-screen").hidden = true;
  renderResources(session);
  renderRoute(session);
  renderUnits(session);
  renderTraits(session);
  renderShop(session);
  renderInvestmentModal(session);
  renderFinish(session);
  renderDrawer(session);
}

function exportRun() {
  if (!appState.session) {
    return;
  }
  const blob = new Blob([JSON.stringify(appState.session, null, 2)], {
    type: "application/json",
  });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `currency-war-${appState.session.session_id.slice(0, 8)}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
}

document.addEventListener("click", async (event) => {
  const actionTarget = event.target.closest("[data-action]");
  if (actionTarget) {
    event.stopPropagation();
    const action = actionTarget.dataset.action;
    if (action === "buy") {
      await runAction({ type: "buy_offer", index: Number(actionTarget.dataset.index) });
    } else if (action === "sell") {
      await runAction({
        type: "sell_unit",
        unit_id: actionTarget.dataset.unitId,
        star: Number(actionTarget.dataset.star),
      });
    } else if (action === "investment") {
      await runAction({
        type: "choose_investment",
        index: Number(actionTarget.dataset.index),
      });
    }
    return;
  }

  const drawerTarget = event.target.closest("[data-drawer]");
  if (drawerTarget) {
    appState.drawer = drawerTarget.dataset.drawer;
    renderDrawer(appState.session);
    return;
  }

  const target = event.target.closest("button");
  if (!target) {
    return;
  }
  if (target.id === "new-run") {
    byId("start-screen").hidden = false;
    return;
  }
  if (target.id === "close-drawer") {
    appState.drawer = null;
    byId("drawer").hidden = true;
    return;
  }
  if (target.id === "toggle-lock") {
    await runAction({ type: "lock_shop", locked: !appState.session.shop_locked });
    return;
  }
  if (target.id === "reroll-shop") {
    await runAction({ type: "reroll_shop" });
    return;
  }
  if (target.id === "buy-xp") {
    await runAction({ type: "buy_xp", count: 1 });
    return;
  }
  if (target.id === "advance-node") {
    await runAction({ type: "advance_node" });
    return;
  }
  if (target.id === "apply-progress") {
    await runAction({
      type: "set_battle_progress",
      progress: Number(byId("battle-progress").value),
    });
    return;
  }
  if (target.id === "export-run") {
    exportRun();
  }
});

async function init() {
  byId("start-form").addEventListener("submit", startRun);
  try {
    appState.meta = await request("/api/meta");
    populateMeta(appState.meta);
  } catch (error) {
    showError(error);
  }
}

init();
