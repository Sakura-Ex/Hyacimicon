const state = {
  meta: null,
  session: null,
  busy: false,
};

const byId = (id) => document.getElementById(id);

const escapeHtml = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");

const formatSigned = (value) => (value > 0 ? `+${value}` : String(value));

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
  }, 4200);
}

function setBusy(busy) {
  state.busy = busy;
  document.body.classList.toggle("is-busy", busy);
}

async function runAction(action) {
  if (!state.session || state.busy) {
    return;
  }
  setBusy(true);
  try {
    state.session = await request(`/api/sessions/${state.session.session_id}/actions`, {
      method: "POST",
      body: JSON.stringify(action),
    });
    render();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

function populateMeta(meta) {
  byId("game-version").textContent = `规则 ${meta.game_version}`;
  byId("validation-status").textContent = `验证状态：${meta.validation}`;
  byId("validation-copy").textContent =
    `当前规则包状态为“${meta.validation}”。界面中的假设值会在创建对局前明确展示。`;

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
          `<option value="${escapeHtml(environment.id)}">${escapeHtml(environment.name)} · ${escapeHtml(environment.description)}</option>`,
      )
      .join("");

  byId("assumptions").innerHTML = meta.assumptions
    .map((assumption) => `<li>${escapeHtml(assumption)}</li>`)
    .join("");
}

async function startRun(event) {
  event.preventDefault();
  if (state.busy) {
    return;
  }
  setBusy(true);
  try {
    state.session = await request("/api/sessions", {
      method: "POST",
      body: JSON.stringify({
        difficulty_id: byId("difficulty").value,
        environment_id: byId("environment").value || null,
        seed: Number(byId("seed").value),
        battle_progress: Number(byId("start-progress").value),
      }),
    });
    byId("empty-state").hidden = true;
    byId("run-view").hidden = false;
    render();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

function renderNodeTrack(session) {
  const resultsByIndex = new Map(
    session.results.map((result) => [result.node_index, result]),
  );
  byId("node-track").innerHTML = session.node_template
    .map((node) => {
      const result = resultsByIndex.get(node.index);
      const current = node.index === session.node.index && !session.finished;
      const status = result ? (result.won ? "won" : "lost") : "";
      const currentClass = current ? "current" : "";
      return `
        <div class="node-dot ${status} ${currentClass}" title="位面 ${node.plane} · ${node.type}">
          ${node.index + 1}
          <strong>${escapeHtml(node.type.slice(0, 4))}</strong>
        </div>
      `;
    })
    .join("");
}

function renderShop(session) {
  byId("shop").innerHTML = session.shop
    .map((offer, index) => {
      if (offer.empty) {
        return '<article class="shop-card empty"><span>空位</span><p>不可购买</p></article>';
      }
      return `
        <article class="shop-card">
          <div>
            <span class="cost">${offer.cost} 金币 · ${escapeHtml(offer.position)}</span>
            <h3>${escapeHtml(offer.name)}</h3>
            <p>${escapeHtml(offer.traits.join(" / ") || "无标签")}</p>
          </div>
          <button data-action="buy" data-index="${index}" ${offer.affordable && session.capabilities.buy_offer ? "" : "disabled"}>
            购买
          </button>
        </article>
      `;
    })
    .join("");

  const lock = byId("toggle-lock");
  lock.textContent = session.shop_locked ? "解除锁定" : "锁定商店";
  lock.disabled = !session.capabilities.lock_shop;
  byId("reroll-shop").disabled = !session.capabilities.reroll_shop;
  byId("buy-xp").disabled = !session.capabilities.buy_xp;
  byId("advance-node").disabled = !session.capabilities.advance_node;
}

function renderUnits(session) {
  const activeKeys = new Set(
    session.active_units.map((unit) => `${unit.unit_id}:${unit.star}`),
  );
  byId("active-units").innerHTML =
    session.active_units
      .map(
        (unit) => `
          <article class="unit-card active">
            <span class="cost">${unit.cost} 金币</span>
            <h3>${escapeHtml(unit.name)} · ${unit.star}★</h3>
            <p>${escapeHtml(unit.position)}</p>
          </article>
        `,
      )
      .join("") || '<p class="muted">暂无上阵角色</p>';

  byId("roster-count").textContent = `${session.roster.length} 个角色实例`;
  byId("roster").innerHTML =
    session.roster
      .map((unit) => {
        const active = activeKeys.has(`${unit.unit_id}:${unit.star}`);
        return `
          <article class="unit-card ${active ? "active" : ""}">
            <span class="cost">${unit.cost} 金币</span>
            <h3>${escapeHtml(unit.name)} · ${unit.star}★</h3>
            <p>${escapeHtml(unit.traits.join(" / ") || "无标签")}</p>
            <button
              class="danger"
              data-action="sell"
              data-unit-id="${escapeHtml(unit.unit_id)}"
              data-star="${unit.star}"
              ${session.capabilities.sell_unit ? "" : "disabled"}
            >出售</button>
          </article>
        `;
      })
      .join("") || '<p class="muted">暂无角色</p>';
}

function renderInvestments(session) {
  const panel = byId("investment-panel");
  const hasOffers = session.investments.offers.length > 0;
  panel.hidden = !hasOffers;
  byId("investment-offers").innerHTML = session.investments.offers
    .map(
      (investment, index) => `
        <article class="choice-card">
          <span class="rarity">${escapeHtml(investment.rarity)}</span>
          <h3>${escapeHtml(investment.name)}</h3>
          <p>${escapeHtml(investment.description)}</p>
          <button
            data-action="investment"
            data-index="${index}"
            ${session.capabilities.choose_investment ? "" : "disabled"}
          >选择</button>
        </article>
      `,
    )
    .join("");

  byId("selected-investments").innerHTML =
    session.investments.selected
      .map(
        (investment) => `
          <div class="stack-item">
            <span class="rarity">${escapeHtml(investment.rarity)}</span>
            <strong>${escapeHtml(investment.name)}</strong>
            <p>${escapeHtml(investment.description)}</p>
          </div>
        `,
      )
      .join("") || '<p class="muted">尚未选择投资策略</p>';

  byId("affixes").innerHTML =
    session.affixes
      .map(
        (affix) => `
          <div class="stack-item">
            <strong>${escapeHtml(affix.name)}</strong>
            <p>${escapeHtml(affix.description)}</p>
          </div>
        `,
      )
      .join("") || '<p class="muted">没有开局词缀</p>';
}

function renderResults(session) {
  byId("results").innerHTML =
    session.results
      .map(
        (result) => `
          <tr>
            <td>${result.node_index + 1}</td>
            <td>${escapeHtml(result.node_type)}</td>
            <td class="${result.won ? "win" : "loss"}">${result.won ? "胜利" : "失败"}</td>
            <td>${result.battle_progress}%</td>
            <td>${formatSigned(result.health_delta)}</td>
            <td>${formatSigned(result.gold_delta)}</td>
          </tr>
        `,
      )
      .join("") || '<tr><td colspan="6" class="muted">尚未结算节点</td></tr>';
}

function renderEvents(session) {
  byId("events").innerHTML = [...session.events]
    .reverse()
    .map(
      (event) => `
        <li>
          <span>#${event.sequence}</span>
          <div>${escapeHtml(event.message)}</div>
        </li>
      `,
    )
    .join("");
}

function renderDebug(session) {
  const enemyRating =
    session.debug.node_enemy_rating === null
      ? "已结束"
      : session.debug.node_enemy_rating.toFixed(2);
  byId("debug-summary").innerHTML = `
    <span>阵容强度：${session.debug.team_power.toFixed(2)}</span>
    <span>节点强度：${enemyRating}</span>
    <span>随机种子：${session.seed}</span>
    <span>战斗提供器：${escapeHtml(session.battle.provider_id)}</span>
  `;
  byId("pool").innerHTML = session.debug.pool
    .map(
      (unit) => `
        <div class="pool-item">
          <span>${escapeHtml(unit.name)}</span>
          <strong>${unit.remaining}</strong>
        </div>
      `,
    )
    .join("");
}

function render() {
  const session = state.session;
  if (!session) {
    return;
  }
  byId("empty-state").hidden = true;
  byId("run-view").hidden = false;
  byId("session-id").textContent = `#${session.session_id.slice(0, 8)}`;
  byId("run-title").textContent = `节点 ${session.node.number} / ${session.node.total}`;
  byId("run-context").textContent =
    `${session.difficulty.name} · ${session.environment.name} · ${session.node.plane} 位面 · ${session.node.type}`;
  byId("phase-label").textContent = {
    operations: "运营阶段",
    investment: "等待选择投资",
    finished: "对局结束",
  }[session.phase];

  byId("health").textContent = `${session.resources.health} / ${session.resources.max_health}`;
  byId("gold").textContent = session.resources.gold;
  const nextXp =
    session.resources.xp_to_next_level === null
      ? "MAX"
      : `${session.resources.xp} / ${session.resources.xp_to_next_level}`;
  byId("level").textContent = `${session.resources.level} · ${nextXp}`;
  byId("streak").textContent = session.resources.streak;
  byId("free-rerolls").textContent = session.resources.free_rerolls;
  byId("progress-status").textContent = `${session.battle.progress_percent}%`;
  byId("battle-progress").value = session.battle.progress_percent;
  byId("progress-form").querySelector("button").disabled =
    !session.capabilities.set_battle_progress;

  const banner = byId("finish-banner");
  banner.hidden = !session.finished;
  if (session.finished) {
    banner.textContent = session.won ? "整局胜利" : "对局结束：生命归零";
  }

  renderNodeTrack(session);
  renderShop(session);
  renderUnits(session);
  renderInvestments(session);
  renderResults(session);
  renderEvents(session);
  renderDebug(session);
}

document.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-action]");
  if (button) {
    const action = button.dataset.action;
    if (action === "buy") {
      await runAction({ type: "buy_offer", index: Number(button.dataset.index) });
    } else if (action === "sell") {
      await runAction({
        type: "sell_unit",
        unit_id: button.dataset.unitId,
        star: Number(button.dataset.star),
      });
    } else if (action === "investment") {
      await runAction({
        type: "choose_investment",
        index: Number(button.dataset.index),
      });
    }
    return;
  }

  const target = event.target.closest("button");
  if (!target) {
    return;
  }
  if (target.id === "toggle-lock") {
    await runAction({ type: "lock_shop", locked: !state.session.shop_locked });
  } else if (target.id === "reroll-shop") {
    await runAction({ type: "reroll_shop" });
  } else if (target.id === "buy-xp") {
    await runAction({ type: "buy_xp", count: 1 });
  } else if (target.id === "advance-node") {
    await runAction({ type: "advance_node" });
  } else if (target.id === "refresh-run") {
    await refreshRun();
  } else if (target.id === "export-run") {
    exportRun();
  }
});

function exportRun() {
  if (!state.session) {
    return;
  }
  const blob = new Blob([JSON.stringify(state.session, null, 2)], {
    type: "application/json",
  });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `currency-war-run-${state.session.session_id.slice(0, 8)}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
}

async function refreshRun() {
  if (!state.session || state.busy) {
    return;
  }
  setBusy(true);
  try {
    state.session = await request(`/api/sessions/${state.session.session_id}`);
    render();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

async function init() {
  byId("start-form").addEventListener("submit", startRun);
  byId("progress-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    await runAction({
      type: "set_battle_progress",
      progress: Number(byId("battle-progress").value),
    });
  });
  try {
    state.meta = await request("/api/meta");
    populateMeta(state.meta);
  } catch (error) {
    showError(error);
  }
}

init();
