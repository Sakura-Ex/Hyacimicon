import { demoFixture, demoPort, demoRules, type DemoFixture, type DemoItem } from "./demo.ts";
import { matchStream } from "./match-stream.ts";
import type { Advice, Feedback, Item } from "./contracts.ts";
import type { SessionPort } from "./match-port.ts";
import type { ChoiceOption, CreateMatchInput, LegalAction, MatchAction, MatchPhase, MatchResult,
  MatchSnapshot, MatchUpdate, MatchVersion, NodeKind, Plane, Preparation, RouteNode, Slot } from "./match-session-contract.ts";

const demoModel = { id: "demo", name: "演示运营模型" };
const zones = ["front", "back", "bench"] as const;

const planes: { plane: Plane; kinds: NodeKind[] }[] = [
  { plane: 1, kinds: ["reward", "reward", "battle", "battle", "supply", "battle", "encounter", "reward", "boss"] },
  { plane: 2, kinds: ["battle", "battle", "supply", "battle", "encounter", "reward", "boss"] },
  { plane: 3, kinds: ["battle", "battle", "supply", "battle", "encounter", "reward", "boss"] },
];
const route: RouteNode[] = planes.flatMap(({ plane, kinds }) => kinds.map((kind, index) => ({
  nodeId: `${plane}-${index + 1}`, kind,
})));
const pools = {
  investmentEnvironment: ["丰收季", "淘金时代", "银·金·彩", "开拓之初", "远行准备", "稳步起步"],
  investmentStrategy: ["积少成多", "有备无患", "乘胜追击", "稳健投资", "厚积薄发", "储备补给"],
  spotlight: ["舞台聚光", "群星共鸣", "终幕序曲"],
  tea: ["随行补给", "小憩片刻"],
  trait: ["稳步开拓", "全速前进"],
};
function options(pool: keyof typeof pools): ChoiceOption[] {
  return pools[pool].map((name, index) => ({
    id: pool + index, optionId: pool + index, name, detail: pool === "spotlight"
      ? "为所选角色激活光环，并增加首领难度。" : "本地演示选项。",
  }));
}
const refreshable = (pool: "investmentEnvironment" | "investmentStrategy") =>
  options(pool).slice(0, 3).map(option => ({ ...option, refreshAvailable: true }));
const definitionId = (item: Item) => item.image.split("/").at(-1)?.split(".")[0] ?? item.name;
const stack = (item: DemoItem) => ({ itemId: item.itemId, definitionId: definitionId(item), count: item.count });
const component = (id: string) => /^35020[1-6]$/.test(id);
const sameSlot = (a: Slot, b: Slot) => a.zone === b.zone && a.index === b.index;
const fielded = (p: Preparation) => p.board.pieces.filter(piece => piece.slot.zone !== "bench");
function bench(p: Preparation): Slot | undefined {
  for (let index = 0; index < p.board.bench; index++) {
    const slot: Slot = { zone: "bench", index };
    if (!p.board.pieces.some(piece => sameSlot(piece.slot, slot))) return slot;
  }
}
function preparation(m: DemoFixture): Preparation {
  return {
    resources: { health: m.health, maxHealth: 100, gold: m.gold, hotStreak: m.streak,
      interestCap: 5, level: 8, experience: 68, experienceToNextLevel: 72 },
    shop: { locked: false, freeRefreshes: 1, slots: m.offers.map(o => ({ offerId: o.offerId, unitId: o.unit.id, price: o.cost })) },
    board: { front: m.frontCapacity, back: m.backCapacity, bench: m.benchCapacity, active: m.capacity,
      pieces: zones.flatMap(zone => m[zone].map(piece => ({ pieceId: piece.pieceId, unitId: piece.id,
        stars: piece.stars, slot: { zone, index: piece.slot }, equipment: piece.gear.map(stack) }))) },
    inventory: [...m.inventory, ...m.consumables].map(stack),
    crystals: (["white", "blue", "gold", "prismatic"] as const).map(quality => ({ quality, crystalId: quality, count: 1 })),
    traits: m.traits.flatMap(t => t.progress ? [{ traitId: definitionId(t), ...t.progress,
      activeThreshold: t.progress.thresholds.filter(n => n <= (t.progress?.count ?? 0)).at(-1) ?? null }] : []),
    selectedInvestments: [], selectedAuras: [], revealedEnemies: [],
    mechanisms: [
      { kind: "aha", counters: [{ id: "笑点", value: 2 }], choices: [{ choiceId: "aha", mode: "once", selectedOptionId: null, options: options("trait") }] },
      { kind: "wishPower", counters: [{ id: "愿力", value: 4 }], choices: [{ choiceId: "wish", mode: "changeable", selectedOptionId: "trait0", options: options("trait") }] },
      { kind: "trailblazeShop", counters: [{ id: "剩余兑换", value: 1 }], choices: [] },
      { kind: "chimeraGarden", counters: [{ id: "奇兽", value: 2 }], choices: [] },
    ],
  };
}

interface Session {
  snapshot: MatchSnapshot;
  fixture: DemoFixture;
  seed: string;
  draw: number;
  attempt: number;
  battleRoll: number;
  tea?: ChoiceOption;
  feedback?: Feedback;
  listeners: Set<(event: MatchUpdate) => void>;
  timer?: ReturnType<typeof setTimeout>;
}

function random(session: Session, size: number) {
  let hash = 2166136261;
  for (const char of session.seed + ":" + session.draw++) hash = Math.imul(hash ^ char.charCodeAt(0), 16777619);
  return (hash >>> 0) % size;
}
function supply(session: Session) {
  const { offers, inventory } = session.fixture;
  return offers.map((_, index) => ({ optionId: "supply-" + index,
    unitId: offers[(index + random(session, offers.length)) % offers.length].unit.id,
    itemDefinitionIds: [definitionId(inventory[random(session, inventory.length)])],
  }));
}
function enter(session: Session, index: number, strategyChosen = false) {
  const s = session.snapshot;
  const p = s.preparation;
  const node = s.route[index];
  if (!p || !node) return;
  s.position = { ...node, index, enteredTriggerNodes: s.position?.enteredTriggerNodes ?? 0 };
  const selectionId = node.nodeId;
  if (["1-3", "2-2", "3-2"].includes(selectionId) && !strategyChosen) {
    const qualities = ["silver", "gold", "prismatic"] as const;
    const quality = random(session, qualities.length);
    s.phase = { kind: "investmentStrategy", selectionId, view: "selection",
      options: refreshable("investmentStrategy").map((option, index) => ({
        ...option, quality: qualities[s.environment?.id === "investmentEnvironment2" ? index : quality],
      })) };
    return;
  }
  if (node.kind === "supply") {
    s.phase = { kind: "supply", selectionId, view: "selection", refreshAvailable: true, options: supply(session) };
    return;
  }
  s.position.enteredTriggerNodes++;
  p.resources.gold++;
  const opponent = s.opponents.find(group => group.plane === Number(node.nodeId[0]));
  p.revealedEnemies = opponent ? node.kind === "boss" ? opponent.bosses : opponent.enemies : [];
  s.phase = node.nodeId === "2-6" ? { kind: "teaBreak", selectionId, options: options("tea") } :
    { kind: "preparation", spotlightAvailable: node.kind === "boss" && node.nodeId[0] !== "3", openTraitChoice: null };
}

function legal(s: MatchSnapshot) {
  const actions: LegalAction[] = [];
  function add(action: MatchAction, goldCost?: number, source?: LegalAction["source"]) {
    actions.push({ actionId: `${s.revision}-${actions.length}`, action, goldCost, source });
  }
  const { phase, preparation: p } = s;
  if (phase.kind === "investmentEnvironment" || phase.kind === "investmentStrategy" && phase.view === "selection") {
    for (const { optionId, refreshAvailable } of phase.options) {
      const { selectionId } = phase;
      if (refreshAvailable) add({ kind: "refreshChoice", choice: phase.kind, selectionId, optionId });
      add({ kind: phase.kind === "investmentEnvironment" ? "chooseInvestmentEnvironment" : "chooseInvestmentStrategy", selectionId, optionId });
    }
  }
  if ("view" in phase) add({ kind: phase.view === "selection" ? "returnToPreparation" : "returnToSelection", selectionId: phase.selectionId });
  if (phase.kind === "supply" && phase.view === "selection") {
    if (phase.refreshAvailable) add({ kind: "refreshSupply", selectionId: phase.selectionId });
    for (const { optionId } of phase.options) add({ kind: "takeSupply", selectionId: phase.selectionId, optionId });
  }
  if (phase.kind === "spotlight" && phase.view === "selection" && p) {
    for (const { optionId } of phase.options) {
      if (p.resources.gold >= phase.refreshPrice) add({ kind: "refreshChoice", choice: "spotlight", selectionId: phase.selectionId, optionId }, phase.refreshPrice);
      for (const pieceId of phase.eligiblePieceIds) add({ kind: "chooseSpotlight", selectionId: phase.selectionId, optionId, pieceId });
    }
  }
  if (phase.kind === "teaBreak") for (const { optionId } of phase.options) add({ kind: "chooseTeaBreak", selectionId: phase.selectionId, optionId });
  if (phase.kind === "teaBreakReward") add({ kind: "claimTeaBreakReward", selectionId: phase.selectionId });
  if (phase.kind === "battle" || phase.kind === "battleUnavailable") {
    add({ kind: "restartBattle" });
    add({ kind: "leaveBattle" });
  }
  if (phase.kind === "battleAway") add({ kind: "reenterBattle" });
  if (!p) return actions;
  const normal = phase.kind === "preparation" && !phase.openTraitChoice;
  const returned = "view" in phase && phase.view === "preparation";
  const vacancy = bench(p);
  if ((normal || returned) && vacancy) for (const { crystalId } of p.crystals) add({ kind: "openCrystal", crystalId });
  if (normal || phase.kind === "battleAway" || phase.kind === "spotlight" && returned) {
    const active = fielded(p).length;
    for (const piece of p.board.pieces) for (const zone of zones) for (let index = 0; index < p.board[zone]; index++) {
      const destination = { zone, index };
      if (sameSlot(piece.slot, destination)) continue;
      if (piece.slot.zone === "bench" && zone !== "bench" && active >= p.board.active &&
          !p.board.pieces.some(other => sameSlot(other.slot, destination))) continue;
      add({ kind: "move", pieceId: piece.pieceId, destination });
    }
  }
  if (phase.kind !== "preparation") return actions;
  for (const mechanism of p.mechanisms) for (const choice of mechanism.choices) {
    const key = { mechanism: mechanism.kind, choiceId: choice.choiceId };
    const source = { kind: "trait" as const, traitId: mechanism.kind, mechanism: mechanism.kind };
    const open = phase.openTraitChoice;
    if (open) {
      if (open.mechanism !== key.mechanism || open.choiceId !== key.choiceId) continue;
      for (const { optionId } of choice.options) add({ kind: "selectTraitOption", ...key, optionId }, undefined, source);
      if (open.draftOptionId) add({ kind: "confirmTraitChoice", ...key }, undefined, source);
    } else if (choice.mode === "changeable") add({ kind: "openTraitChoice", ...key }, undefined, source);
    else if (!choice.selectedOptionId) for (const { optionId } of choice.options) add({ kind: "chooseMechanism", ...key, optionId }, undefined, source);
  }
  if (!normal) return actions;
  for (const piece of p.board.pieces) {
    add({ kind: "sell", pieceId: piece.pieceId });
    for (const item of p.inventory) {
      if (item.definitionId === "350102" && piece.equipment.length) {
        add({ kind: "useItem", itemId: item.itemId, target: { kind: "piece", pieceId: piece.pieceId } });
      }
      if (item.definitionId === "350103") for (const gear of piece.equipment) {
        add({ kind: "useItem", itemId: item.itemId, target: { kind: "item", itemId: gear.itemId } });
      }
      if ((component(item.definitionId) || item.definitionId === "demo-combined") &&
          (piece.equipment.length < 3 || component(item.definitionId) && piece.equipment.some(gear => component(gear.definitionId)))) {
        add({ kind: "equip", itemId: item.itemId, pieceId: piece.pieceId });
      }
    }
  }
  const gear = p.inventory.filter(item => component(item.definitionId));
  for (const [index, item] of gear.entries()) for (const other of gear.slice(index + 1)) {
    add({ kind: "combine", itemId: item.itemId, otherItemId: other.itemId });
  }
  for (const offer of p.shop.slots) if (offer && vacancy && p.resources.gold >= offer.price) add({ kind: "buy", offerId: offer.offerId }, offer.price);
  const refresh = p.shop.freeRefreshes > 0 ? 0 : 2;
  if (p.resources.gold >= refresh) add({ kind: "refreshShop" }, refresh);
  if (p.resources.gold >= 4 && p.resources.experienceToNextLevel !== null) add({ kind: "buyExperience" }, 4);
  add({ kind: "lockShop", locked: !p.shop.locked });
  if (phase.spotlightAvailable && fielded(p).length) add({ kind: "openSpotlight" });
  add({ kind: "startBattle" });
  return actions;
}

function experience(p: Preparation, gain: number) {
  const r = p.resources;
  if (r.experienceToNextLevel === null) return;
  r.experience += gain;
  if (r.experience < r.experienceToNextLevel) return;
  r.experience -= r.experienceToNextLevel;
  r.level++;
  p.board.active = Math.min(p.board.front + p.board.back, p.board.active + 1);
  r.experienceToNextLevel = r.level === 9 ? 84 : null;
}
function settle(session: Session, source: "battle" | "teaBreak") {
  const s = session.snapshot;
  const { preparation: p, position } = s;
  if (!p || !position) return;
  const r = p.resources;
  const healthBefore = r.health;
  const healthLost = source === "battle" && session.battleRoll === 0 ? 15 : 0;
  const fixedRecovery = 2;
  r.health = Math.max(0, Math.min(r.maxHealth, r.health - healthLost + fixedRecovery));
  const baseGold = 5;
  const interestGold = Math.min(r.interestCap, Math.floor(r.gold / 10));
  r.hotStreak = healthLost ? 0 : r.hotStreak + 1;
  const streakGold = Math.min(3, r.hotStreak);
  r.gold += baseGold + interestGold + streakGold;
  const experienceGained = source === "teaBreak" ? 0 : 2;
  experience(p, experienceGained);
  const reward = { crystalId: "white", quality: "white" as const, count: 2 };
  const crystal = p.crystals.find(crystal => crystal.crystalId === reward.crystalId);
  if (crystal) crystal.count += reward.count;
  else p.crystals.push({ ...reward });
  s.latestSettlement = { nodeId: position.nodeId, source, healthBefore, healthAfter: r.health, healthLost,
    fixedRecovery, otherHealthDelta: 0, baseGold, interestGold, streakGold, hotStreakAfter: r.hotStreak,
    experienceGained, crystalsGranted: [reward] };
  if (r.health <= 0) s.phase = { kind: "finished", reason: "healthDepleted" };
  else if (position.index === route.length - 1) s.phase = { kind: "finished", reason: "cleared" };
  else enter(session, position.index + 1);
}

function take(p: Preparation, itemId: string, newId: string) {
  const index = p.inventory.findIndex(item => item.itemId === itemId);
  const item = p.inventory[index];
  if (!item) return;
  if (item.count === 1) return p.inventory.splice(index, 1)[0];
  item.count--;
  return { ...item, itemId: newId, count: 1 };
}
function openSpotlight(p: Preparation, nodeId: string): MatchPhase {
  return { kind: "spotlight", selectionId: nodeId, view: "selection", options: options("spotlight").slice(0, 2),
    eligiblePieceIds: fielded(p).map(piece => piece.pieceId), refreshCount: 0, discountedRefreshLimit: 6, refreshPrice: 1 };
}
function apply(session: Session, { action: a, goldCost }: LegalAction) {
  const s = session.snapshot;
  const { phase, preparation: p, position } = s;
  if (a.kind === "chooseInvestmentEnvironment" && phase.kind === "investmentEnvironment") {
    s.environment = phase.options.find(option => option.optionId === a.optionId) ?? null;
    s.preparation = preparation(session.fixture);
    enter(session, 0);
    return;
  }
  if (a.kind === "refreshChoice" && (phase.kind === "investmentEnvironment" || phase.kind === "investmentStrategy" || phase.kind === "spotlight")) {
    const previous = phase.options.find(option => option.optionId === a.optionId);
    const alternatives = options(phase.kind).filter(option => !phase.options.some(active => active.id === option.id));
    if (previous && alternatives.length) {
      const { id, name, detail } = alternatives[random(session, alternatives.length)];
      Object.assign(previous, { id, name, detail });
      if ("refreshAvailable" in previous) previous.refreshAvailable = false;
    }
    if (phase.kind === "spotlight" && p) {
      p.resources.gold -= goldCost ?? 0;
      phase.refreshCount++;
      phase.refreshPrice = phase.refreshCount < phase.discountedRefreshLimit ? 1 : 2;
    }
    return;
  }
  if (!p || !position) return;
  p.resources.gold -= goldCost ?? 0;
  const nextId = `r${s.revision + 1}`;
  const piece = "pieceId" in a ? p.board.pieces.find(piece => piece.pieceId === a.pieceId) : undefined;
  function recruit(board: Preparation, unitId: string) {
    const slot = bench(board);
    if (slot) board.board.pieces.push({ unitId, slot, pieceId: nextId, stars: 1, equipment: [] });
  }
  switch (a.kind) {
    case "returnToPreparation": if ("view" in phase) phase.view = "preparation"; break;
    case "returnToSelection":
      if ("view" in phase) phase.view = "selection";
      if (phase.kind === "spotlight") phase.eligiblePieceIds = fielded(p).map(piece => piece.pieceId);
      break;
    case "chooseInvestmentStrategy": {
      if (phase.kind !== "investmentStrategy") break;
      const option = phase.options.find(o => o.optionId === a.optionId);
      if (option) p.selectedInvestments.push(option);
      enter(session, position.index, true);
      break;
    }
    case "refreshSupply":
      if (phase.kind === "supply") { phase.options = supply(session); phase.refreshAvailable = false; }
      break;
    case "takeSupply": {
      if (phase.kind !== "supply") break;
      const option = phase.options.find(o => o.optionId === a.optionId);
      if (!option) break;
      // A full preview bench receives the group's items and has no slot for its recruit.
      recruit(p, option.unitId);
      p.inventory.push(...option.itemDefinitionIds.map((definitionId, i) => ({ definitionId, itemId: nextId + "-" + i, count: 1 })));
      enter(session, position.index + 1);
      break;
    }
    case "openSpotlight": s.phase = openSpotlight(p, "aura-" + position.nodeId); break;
    case "chooseSpotlight": {
      if (phase.kind !== "spotlight") break;
      const option = phase.options.find(o => o.optionId === a.optionId);
      if (option) {
        for (const aura of p.selectedAuras) if (aura.pieceId === a.pieceId) aura.effectActive = false;
        p.selectedAuras.push({ ...option, pieceId: a.pieceId, effectActive: true, difficultyIncrease: 1 });
      }
      s.phase = { kind: "preparation", spotlightAvailable: false, openTraitChoice: null };
      break;
    }
    case "chooseTeaBreak":
      if (phase.kind === "teaBreak") {
        session.tea = phase.options.find(option => option.optionId === a.optionId);
        s.phase = { kind: "teaBreakReward", selectionId: a.selectionId, selectedOptionId: a.optionId };
      }
      break;
    case "claimTeaBreakReward": settle(session, "teaBreak"); break;
    case "move":
      if (piece) {
        const other = p.board.pieces.find(other => sameSlot(other.slot, a.destination));
        if (other) other.slot = piece.slot;
        piece.slot = a.destination;
      }
      break;
    case "sell":
      if (piece) {
        p.inventory.push(...piece.equipment);
        p.board.pieces.splice(p.board.pieces.indexOf(piece), 1);
        p.resources.gold++;
      }
      break;
    case "buy": {
      const index = p.shop.slots.findIndex(offer => offer?.offerId === a.offerId);
      const offer = p.shop.slots[index];
      if (offer) { recruit(p, offer.unitId); p.shop.slots[index] = null; }
      break;
    }
    case "refreshShop": {
      if (p.shop.freeRefreshes > 0) p.shop.freeRefreshes--;
      const offers = session.fixture.offers;
      const offset = 1 + random(session, offers.length - 1);
      p.shop.slots = offers.map((_, index) => {
        const offer = offers[(index + offset) % offers.length];
        return { offerId: nextId + "-" + index, unitId: offer.unit.id, price: offer.cost };
      });
      break;
    }
    case "buyExperience": experience(p, 4); break;
    case "lockShop": p.shop.locked = a.locked; break;
    case "openCrystal": {
      const crystal = p.crystals.find(crystal => crystal.crystalId === a.crystalId);
      if (crystal) {
        if (--crystal.count === 0) p.crystals.splice(p.crystals.indexOf(crystal), 1);
        p.resources.gold += 2 + random(session, 2);
      }
      break;
    }
    case "equip": {
      if (!piece) break;
      const item = take(p, a.itemId, nextId);
      if (!item) break;
      const other = piece.equipment.find(gear => component(gear.definitionId));
      if (component(item.definitionId) && other) other.definitionId = "demo-combined";
      else piece.equipment.push(item);
      break;
    }
    case "combine":
      take(p, a.itemId, nextId);
      take(p, a.otherItemId, nextId + "-other");
      p.inventory.push({ itemId: nextId, definitionId: "demo-combined", count: 1 });
      break;
    case "useItem": {
      const target = a.target;
      if (target.kind === "piece") {
        const owner = p.board.pieces.find(piece => piece.pieceId === target.pieceId);
        if (owner) { p.inventory.push(...owner.equipment); owner.equipment = []; }
      } else {
        for (const piece of p.board.pieces) {
          const index = piece.equipment.findIndex(item => item.itemId === target.itemId);
          if (index < 0) continue;
          const [item] = piece.equipment.splice(index, 1);
          p.inventory.push({ ...item, definitionId: "35020" + (1 + random(session, 6)) });
          take(p, a.itemId, nextId);
          break;
        }
      }
      break;
    }
    case "chooseMechanism": case "openTraitChoice": case "selectTraitOption": case "confirmTraitChoice": {
      if (phase.kind !== "preparation") break;
      const choice = p.mechanisms.find(m => m.kind === a.mechanism)?.choices.find(c => c.choiceId === a.choiceId);
      if (!choice) break;
      if (a.kind === "chooseMechanism") choice.selectedOptionId = a.optionId;
      if (a.kind === "openTraitChoice") phase.openTraitChoice = { mechanism: a.mechanism, choiceId: a.choiceId, draftOptionId: choice.selectedOptionId };
      if (a.kind === "selectTraitOption" && phase.openTraitChoice) phase.openTraitChoice.draftOptionId = a.optionId;
      if (a.kind === "confirmTraitChoice" && phase.openTraitChoice) {
        choice.selectedOptionId = phase.openTraitChoice.draftOptionId;
        phase.openTraitChoice = null;
      }
      break;
    }
    case "startBattle": case "restartBattle": case "reenterBattle":
      if (a.kind !== "restartBattle") session.battleRoll = random(session, 5);
      s.phase = { kind: "battle", attemptId: "attempt-" + ++session.attempt };
      break;
    case "leaveBattle": s.phase = { kind: "battleAway" }; break;
  }
}

const ok = <T>(value: T): MatchResult<T> => ({ ok: true, value });
const missing = (): MatchResult<never> => ({ ok: false, error: { code: "matchNotFound", message: "找不到该演示对局。" } });
function stale(s: MatchSnapshot, input: MatchVersion): MatchResult<never> | undefined {
  if (s.revision !== input.revision) return { ok: false, error: { code: "revisionConflict", currentRevision: s.revision } };
}

export function demoSessionPort(): SessionPort & {
  preview(input: CreateMatchInput, scenario: string, holdBattle?: boolean): Promise<MatchResult<MatchSnapshot>>;
  dispose(): void;
} {
  const sessions = new Map<string, Session>();
  let counter = 0;
  function publish(session: Session, event: MatchUpdate) {
    for (const listener of session.listeners) listener(structuredClone(event));
  }
  function commit(session: Session) {
    session.snapshot.revision++;
    session.snapshot.legalActions = legal(session.snapshot);
  }
  function schedule(session: Session) {
    clearTimeout(session.timer);
    const s = session.snapshot;
    if (s.phase.kind === "battle") {
      const { attemptId } = s.phase;
      session.timer = setTimeout(() => {
        if (s.phase.kind !== "battle" || s.phase.attemptId !== attemptId) return;
        settle(session, "battle");
        commit(session);
        if (s.latestSettlement) publish(session, { kind: "battleSettled", matchId: s.matchId, revision: s.revision, attemptId, settlement: s.latestSettlement });
        schedule(session);
      }, 5000);
    } else if (s.controller.kind === "model" && s.legalActions.length) {
      session.timer = setTimeout(() => {
        if (s.controller.kind !== "model") return;
        const priority = ["confirmTraitChoice", "chooseInvestmentEnvironment", "chooseInvestmentStrategy", "takeSupply",
          "chooseSpotlight", "chooseTeaBreak", "claimTeaBreakReward", "reenterBattle", "restartBattle", "returnToSelection", "startBattle"];
        const order = s.phase.kind === "preparation" && s.revision % 3 === 0 ? ["buy", ...priority] : priority;
        const entry = order.map(kind => s.legalActions.find(entry => entry.action.kind === kind)).find(entry => entry);
        if (entry) execute(session, entry, s.controller);
      }, 700);
    }
  }
  function execute(session: Session, entry: LegalAction, actor: MatchSnapshot["controller"]) {
    apply(session, entry);
    commit(session);
    const { matchId, revision } = session.snapshot;
    publish(session, { kind: "actionApplied", matchId, revision, actor, action: entry.action });
    schedule(session);
    return ok(structuredClone(session.snapshot));
  }
  async function create(input: CreateMatchInput, scenario?: string, holdBattle = false): Promise<MatchResult<MatchSnapshot>> {
    if (input.rulesVersion !== demoRules) return { ok: false, error: { code: "rulesUnavailable", message: "请选择演示规则版本。" } };
    if (!Number.isSafeInteger(input.enemyDifficulty) || input.enemyDifficulty < 1 || input.enemyDifficulty > 10 ||
        !Number.isSafeInteger(input.affixCount) || input.affixCount < 0 || input.affixCount > 3 ||
        input.randomness.kind === "seeded" && !input.randomness.seed.trim()) {
      return { ok: false, error: { code: "invalidInput", message: "请检查敌人难度、词条数量和种子。" } };
    }
    const fixture = demoFixture(scenario);
    const { enemies } = await demoPort().battleCatalogue();
    const snapshot: MatchSnapshot = {
      matchId: "demo-" + ++counter, accountId: input.accountId, accountConfigVersion: "demo", revision: 0,
      rulesVersion: input.rulesVersion, controller: { kind: "manual" }, route, position: null,
      phase: { kind: "investmentEnvironment", selectionId: "opening", options: refreshable("investmentEnvironment") },
      rival: { id: "rival", name: "演示对手", detail: `难度 ${input.enemyDifficulty}` },
      opponents: planes.map(({ plane }) => ({ plane, enemies: [enemies[0]], bosses: [enemies[1]] })),
      affixes: ["严苛考核", "逆风", "压力测试"].slice(0, input.affixCount).map((name, index) => ({
        id: "affix-" + index, name, detail: "演示负面词条。",
      })),
      companion: { id: "cat", name: "绷带猫", detail: "装备补给" }, environment: null,
      preparation: null, latestSettlement: null, legalActions: [],
    };
    const session: Session = { snapshot, fixture,
      seed: input.randomness.kind === "seeded" ? input.randomness.seed : crypto.randomUUID(),
      draw: 0, attempt: 0, battleRoll: 1, listeners: new Set() };
    if (scenario && scenario !== "investmentEnvironment") {
      const p = preparation(fixture);
      snapshot.preparation = p;
      snapshot.environment = options("investmentEnvironment")[0];
      const nodeId = scenario === "aura" || scenario === "spotlight" || scenario === "spotlightPreparation" ? "2-7" :
        scenario === "teaBreak" || scenario === "teaBreakReward" ? "2-6" :
        scenario === "supply" || scenario === "supplyPreparation" ? "2-3" :
        scenario === "finished" || scenario === "healthDepleted" ? "3-7" :
        scenario === "planeOne" ? "1-2" : scenario === "planeThree" ? "3-4" : "2-2";
      const index = route.findIndex(node => node.nodeId === nodeId);
      snapshot.position = { ...route[index - 1], index: index - 1,
        enteredTriggerNodes: route.slice(0, index).filter(node => node.kind !== "supply").length };
      const strategy = scenario === "investmentStrategy" || scenario === "investmentStrategyPreparation";
      enter(session, index, !strategy);
      if (scenario === "aura" || scenario === "spotlight" || scenario === "spotlightPreparation") snapshot.phase = openSpotlight(p, "aura-demo");
      if (scenario === "teaBreakReward") apply(session, { actionId: "", action: { kind: "chooseTeaBreak", selectionId: nodeId, optionId: "tea0" } });
      if (scenario === "battle") snapshot.phase = { kind: "battle", attemptId: "attempt-" + ++session.attempt };
      if (scenario === "battleAway") snapshot.phase = { kind: "battleAway" };
      if (scenario === "battleUnavailable") snapshot.phase = { kind: "battleUnavailable", attemptId: "attempt-" + ++session.attempt, message: "演示战斗暂不可用。" };
      if (scenario === "healthDepleted") { p.resources.health = 5; session.battleRoll = 0; }
      if (scenario === "finished" || scenario === "healthDepleted" || strategy) {
        if (strategy) {
          snapshot.position = { ...route[index - 1], index: index - 1, enteredTriggerNodes: snapshot.position.enteredTriggerNodes };
        }
        settle(session, "battle");
      }
      if (scenario.endsWith("Preparation") && "view" in snapshot.phase) snapshot.phase.view = "preparation";
      if (scenario === "traitChoice") {
        apply(session, { actionId: "", action: { kind: "openTraitChoice", mechanism: "wishPower", choiceId: "wish" } });
      }
    }
    snapshot.legalActions = legal(snapshot);
    sessions.set(snapshot.matchId, session);
    if (!holdBattle || snapshot.phase.kind !== "battle") schedule(session);
    return ok(structuredClone(snapshot));
  }
  return {
    createMatch: input => create(input),
    preview: create,
    dispose() {
      for (const session of sessions.values()) {
        clearTimeout(session.timer);
        session.listeners.clear();
      }
      sessions.clear();
    },
    async getMatch({ matchId }) {
      const session = sessions.get(matchId);
      return session ? ok(structuredClone(session.snapshot)) : missing();
    },
    async executeAction(input) {
      const session = sessions.get(input.matchId);
      if (!session) return missing();
      const s = session.snapshot;
      const error = stale(s, input);
      if (error) return error;
      if (s.controller.kind !== "manual") return { ok: false, error: { code: "controllerMismatch", controller: s.controller } };
      const entry = s.legalActions.find(entry => entry.actionId === input.actionId);
      return entry ? execute(session, entry, s.controller) : { ok: false, error: { code: "actionUnavailable", phase: s.phase.kind } };
    },
    async setController(input) {
      const session = sessions.get(input.matchId);
      if (!session) return missing();
      const s = session.snapshot;
      const error = stale(s, input);
      if (error) return error;
      if (input.controller.kind === "model" && input.controller.modelId !== demoModel.id) {
        return { ok: false, error: { code: "invalidInput", message: "该演示模型不可用。" } };
      }
      if (s.controller.kind === input.controller.kind) return ok(structuredClone(s));
      s.controller = { ...input.controller };
      commit(session);
      publish(session, { kind: "controllerChanged", matchId: s.matchId, revision: s.revision, controller: s.controller });
      if (s.phase.kind !== "battle") schedule(session);
      return ok(structuredClone(s));
    },
    async subscribeMatch({ matchId }, signal) {
      const session = sessions.get(matchId);
      if (!session) return missing();
      const stop = () => session.listeners.delete(stream.push);
      if (signal?.aborted) return ok(matchStream<MatchUpdate>(signal).updates());
      const stream = matchStream<MatchUpdate>(signal, stop);
      session.listeners.add(stream.push);
      stream.push({ kind: "snapshot", snapshot: structuredClone(session.snapshot) });
      return ok(stream.updates());
    },
    async matchInsights(input) {
      const session = sessions.get(input.matchId);
      if (!session) return missing();
      const s = session.snapshot;
      const error = stale(s, input);
      if (error) return error;
      const r = s.preparation?.resources;
      const advice: Advice[] = s.phase.kind === "preparation" && r ? [
        { id: "hold", title: "保留金币", reason: "开战前可查看预计利息。", expected: { gold: r.gold },
          effects: [["预计利息", String(Math.min(r.interestCap, Math.floor(r.gold / 10)))]] },
        { id: "refresh", title: "刷新商店", reason: "更换五格报价，寻找其他角色。", expected: {},
          effects: [["本次刷新", s.preparation?.shop.freeRefreshes ? "免费" : "2 金币"]] },
      ] : [];
      return ok({ advice, models: [demoModel], selectedTea: session.tea,
        actions: advice.length ? [{ id: "recruit", title: "招募", expected: {} }, ...advice] : [],
        feedbackFields: r ? [{ key: "gold", label: "剩余金币", current: r.gold, min: 0 },
          { key: "health", label: "整局血量", current: r.health, min: 0, max: r.maxHealth }] : [],
        lastAction: session.feedback?.action,
      });
    },
    async saveSessionFeedback(input) {
      const session = sessions.get(input.matchId);
      if (!session) return missing();
      const s = session.snapshot;
      const error = stale(s, input);
      if (error) return error;
      if (s.controller.kind !== "manual") return { ok: false, error: { code: "controllerMismatch", controller: s.controller } };
      const { gold, health } = input.observed;
      if (s.phase.kind !== "preparation" || !["recruit", "hold", "refresh"].includes(input.action) ||
          !Number.isSafeInteger(gold) || gold < 0 || !Number.isSafeInteger(health) || health < 0 || health > 100) {
        return { ok: false, error: { code: "invalidInput", message: "请填写有效的金币与血量。" } };
      }
      session.feedback = structuredClone(input);
      return ok(null);
    },
  };
}
