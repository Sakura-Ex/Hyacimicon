import assert from "node:assert/strict";
import test from "node:test";
import { demoMatchCatalogue, demoPort, demoRules } from "../src/demo.ts";
import { demoSessionPort } from "../src/demo-match.ts";
import { httpSessionPort } from "../src/match-port.ts";
import { actionsForDrop } from "../src/match-drag.ts";
import { matchSession } from "../src/match-session.ts";

const input = { accountId: "preview", rulesVersion: demoRules, enemyDifficulty: 8, affixCount: 2,
  randomness: { kind: "seeded", seed: "regression" } };
function value(result) {
  assert.equal(result.ok, true, JSON.stringify(result.error));
  return result.value;
}
function find(snapshot, kind, predicate = () => true) {
  const action = snapshot.legalActions.find(entry => entry.action.kind === kind && predicate(entry.action));
  assert.ok(action, kind + " must be available");
  return action;
}
async function execute(port, snapshot, kind, predicate) {
  return value(await port.executeAction({ matchId: snapshot.matchId, revision: snapshot.revision,
    actionId: find(snapshot, kind, predicate).actionId }));
}
const kinds = s => [...new Set(s.legalActions.map(entry => entry.action.kind))].sort();

test("each preview character references its own full portrait", async () => {
  const port = demoPort();
  const { accounts: [account] } = await port.accounts();
  const { allies } = await port.battleCatalogue();
  assert.equal(account.characters.find(c => c.id === "1310").portrait, "/portraits/1310.png");
  for (const character of [...account.characters, ...allies]) {
    assert.equal(character.portrait, `/portraits/${character.id}.png`);
  }
});

test("standard battle uses its selected lineup independently of a match", async () => {
  const port = demoPort();
  const { initial, enemies } = await port.battleCatalogue();
  const selected = { ...initial, allyIds: ["1409"], enemyIds: enemies.map(unit => unit.id) };
  const frame = await port.battle(selected);
  assert.equal(frame.kind, "preview");
  assert.deepEqual(frame.units.map(unit => unit.id), [...selected.allyIds, ...selected.enemyIds]);
  assert.ok(frame.units.every(unit => !unit.gear.length));
  await assert.rejects(port.battle({ ...selected, allyIds: [] }), /选择参战单位/);
  await assert.rejects(port.battle({ ...selected, enemyIds: ["invalid"] }), /未知参战单位/);
});

test("accounts can each create a match before entering the 23-node route", async () => {
  const port = demoSessionPort();
  const opening = value(await port.createMatch(input));
  const another = value(await port.createMatch({ ...input, accountId: "another" }));
  assert.notEqual(opening.matchId, another.matchId);
  assert.equal(opening.position, null);
  assert.equal(opening.preparation, null);
  assert.equal(opening.phase.kind, "investmentEnvironment");
  assert.equal(opening.route.length, 23);
  assert.deepEqual([1, 2, 3].map(plane => opening.route.filter(node => node.nodeId.startsWith(plane + "-")).length), [9, 7, 7]);
  assert.deepEqual(opening.route.filter(node => node.kind === "supply").map(node => node.nodeId), ["1-5", "2-3", "3-3"]);
  const firstSlot = opening.phase.options[0];
  const refreshed = await execute(port, opening, "refreshChoice", a => a.optionId === firstSlot.optionId);
  assert.equal(refreshed.phase.options[0].refreshAvailable, false);
  assert.notEqual(refreshed.phase.options[0].name, firstSlot.name);
  assert.equal(refreshed.legalActions.filter(entry => entry.action.kind === "refreshChoice").length, 2);
  const entered = await execute(port, refreshed, "chooseInvestmentEnvironment");
  assert.equal(entered.position.nodeId, "1-1");
  assert.equal(entered.position.enteredTriggerNodes, 1);
  assert.equal(entered.phase.kind, "preparation");
  assert.equal(value(await port.getMatch({ matchId: another.matchId })).revision, another.revision);
});

test("selected accounts restore their match and do not reuse another account's snapshot", async () => {
  const port = demoSessionPort();
  const session = matchSession(port, () => {});
  const first = input.accountId;
  const second = "second";
  await session.enter(first);
  await session.create(input);
  const original = session.snapshot.matchId;
  await session.enter(second);
  assert.equal(session.snapshot, undefined);
  await session.create({ ...input, accountId: second });
  assert.notEqual(session.snapshot.matchId, original);
  await session.enter(first);
  assert.equal(session.snapshot.matchId, original);
  session.select();
});

test("starting over from a terminal preview keeps setup open after tab navigation", async () => {
  const port = demoSessionPort();
  const session = matchSession(port, () => {}, accountId => port.preview({ ...input, accountId }, "finished"));
  await session.enter(input.accountId);
  assert.equal(session.snapshot.phase.kind, "finished");
  session.newMatch();
  await session.enter(input.accountId);
  assert.equal(session.snapshot, undefined);
  await session.create(input);
  assert.equal(session.snapshot.phase.kind, "investmentEnvironment");
  session.select();
});

test("strategy, supply and spotlight keep candidates and expose only their returned-view actions", async () => {
  const port = demoSessionPort();
  for (const scenario of ["investmentStrategy", "supply", "spotlight"]) {
    const before = value(await port.preview(input, scenario));
    const refresh = scenario === "supply" ? "refreshSupply" : "refreshChoice";
    const refreshed = await execute(port, before, refresh);
    const returned = await execute(port, refreshed, "returnToPreparation");
    assert.deepEqual(returned.phase.options, refreshed.phase.options);
    assert.deepEqual(kinds(returned), scenario === "spotlight" ?
      ["move", "openCrystal", "returnToSelection"] : ["openCrystal", "returnToSelection"]);
    const opened = await execute(port, returned, "openCrystal");
    assert.ok(opened.preparation.resources.gold > returned.preparation.resources.gold);
    const back = await execute(port, opened, "returnToSelection");
    assert.deepEqual(back.phase, refreshed.phase);
    if (scenario === "investmentStrategy") {
      assert.equal(back.position.nodeId, "2-2");
      assert.equal(back.latestSettlement.nodeId, "2-1");
      const chosen = await execute(port, back, "chooseInvestmentStrategy");
      assert.equal(chosen.position.enteredTriggerNodes, back.position.enteredTriggerNodes + 1);
      assert.equal(chosen.preparation.resources.gold, back.preparation.resources.gold + 1);
      assert.deepEqual(chosen.latestSettlement, back.latestSettlement);
    } else if (scenario === "supply") {
      const chosen = await execute(port, back, "takeSupply");
      assert.equal(chosen.position.nodeId, "2-4");
      assert.equal(chosen.position.enteredTriggerNodes, back.position.enteredTriggerNodes + 1);
      assert.equal(chosen.preparation.board.pieces.length, back.preparation.board.pieces.length + 1);
    } else {
      assert.ok(back.phase.eligiblePieceIds.length > 1);
      const pieceId = back.phase.eligiblePieceIds[1];
      const chosen = await execute(port, back, "chooseSpotlight", a => a.pieceId === pieceId);
      assert.equal(chosen.preparation.selectedAuras[0].pieceId, pieceId);
      assert.equal(chosen.phase.kind, "preparation");
      assert.equal(chosen.phase.spotlightAvailable, false);
    }
  }
});

test("full board keeps all addressable slots and blocks crystal opening", async () => {
  const port = demoSessionPort();
  const s = value(await port.preview(input, "full"));
  for (const [zone, count] of [["front", 4], ["back", 9], ["bench", 9]]) {
    const pieces = s.preparation.board.pieces.filter(piece => piece.slot.zone === zone);
    assert.deepEqual(pieces.map(piece => piece.slot.index), Array.from({ length: count }, (_, index) => index));
    assert.ok(pieces.every(piece => piece.stars > 0));
    assert.ok(pieces.some(piece => piece.equipment.length > 0));
  }
  assert.equal(new Set(s.preparation.board.pieces.map(piece => piece.pieceId)).size, 22);
  assert.equal(s.preparation.traits.length, 14);
  assert.ok(!kinds(s).includes("openCrystal"));
  assert.equal(s.preparation.shop.slots.length, 5);
});

test("roster and inventory drag resolve instance IDs and reject changes from an older revision", async () => {
  const port = demoSessionPort();
  const s = value(await port.preview(input, "stock"));
  const p = s.preparation;
  assert.equal(new Set(p.inventory.map(item => item.itemId)).size, 33);
  const duplicate = p.board.pieces.find(piece => piece.slot.zone === "bench" && piece.unitId === "1204");
  const original = p.board.pieces.find(piece => piece.slot.zone === "front" && piece.unitId === "1204");
  const [gear] = p.inventory;
  const [equip] = actionsForDrop(s.legalActions, { kind: "item", id: gear.itemId }, { pieceId: duplicate.pieceId });
  assert.equal(equip.action.kind, "equip");
  assert.deepEqual(actionsForDrop(s.legalActions, { kind: "item", id: gear.itemId }, { pieceId: "missing" }), []);
  const equipped = value(await port.executeAction({ matchId: s.matchId, revision: s.revision, actionId: equip.actionId }));
  assert.deepEqual(equipped.preparation.board.pieces.find(piece => piece.pieceId === original.pieceId), original);
  assert.ok(equipped.preparation.board.pieces.find(piece => piece.pieceId === duplicate.pieceId).equipment.some(item => item.itemId === gear.itemId));
  assert.equal((await port.executeAction({ matchId: s.matchId, revision: s.revision, actionId: equip.actionId })).error.code, "revisionConflict");
  const [move] = actionsForDrop(equipped.legalActions, { kind: "piece", id: duplicate.pieceId }, { slot: original.slot });
  const moved = value(await port.executeAction({ matchId: s.matchId, revision: equipped.revision, actionId: move.actionId }));
  assert.deepEqual(moved.preparation.board.pieces.find(piece => piece.pieceId === original.pieceId).slot, duplicate.slot);
  assert.deepEqual(moved.preparation.board.pieces.find(piece => piece.pieceId === duplicate.pieceId).slot, original.slot);
  const empty = { zone: "bench", index: 8 };
  const emptyMove = actionsForDrop(moved.legalActions, { kind: "piece", id: duplicate.pieceId }, { slot: empty });
  assert.equal(emptyMove.length, 1);
  const movedToEmpty = value(await port.executeAction({ matchId: s.matchId, revision: moved.revision, actionId: emptyMove[0].actionId }));
  assert.deepEqual(movedToEmpty.preparation.board.pieces.find(piece => piece.pieceId === duplicate.pieceId).slot, empty);
  const wrench = p.inventory.find(item => item.definitionId === "350102");
  const removed = await execute(port, movedToEmpty, "useItem", a => a.itemId === wrench.itemId && a.target.pieceId === duplicate.pieceId);
  assert.ok(removed.preparation.inventory.some(item => item.itemId === gear.itemId));
  assert.equal(removed.preparation.board.pieces.find(piece => piece.pieceId === duplicate.pieceId).equipment.length, 0);
});

test("drag distinguishes an equipment pair and its two consumable target kinds", () => {
  const actions = [
    { actionId: "merge", action: { kind: "combine", itemId: "one", otherItemId: "two" } },
    { actionId: "extract", action: { kind: "useItem", itemId: "tool", target: { kind: "item", itemId: "two" } } },
    { actionId: "wrench", action: { kind: "useItem", itemId: "tool", target: { kind: "piece", pieceId: "b" } } },
  ];
  const drop = (id, target) => actionsForDrop(actions, { kind: "item", id }, target).map(entry => entry.actionId);
  assert.deepEqual(drop("two", { itemId: "one" }), ["merge"]);
  assert.deepEqual(drop("tool", { itemId: "two", pieceId: "a" }), ["extract"]);
  assert.deepEqual(drop("tool", { pieceId: "b" }), ["wrench"]);
  assert.deepEqual(drop("tool", { pieceId: "a" }), []);
  assert.deepEqual(drop("one", { itemId: "one" }), []);
});

test("shop holds five positions and shows free refresh and experience costs through legal actions", async () => {
  const port = demoSessionPort();
  const before = value(await port.preview(input, "preparation"));
  const p = before.preparation;
  const offer = p.shop.slots[0];
  const catalogue = demoMatchCatalogue();
  assert.deepEqual(catalogue.units[offer.unitId].traitIds.map(id => catalogue.traits[id].name), ["云上五骁", "召唤"]);
  const bought = await execute(port, before, "buy", a => a.offerId === offer.offerId);
  assert.equal(bought.preparation.shop.slots.length, 5);
  assert.equal(bought.preparation.shop.slots[0], null);
  assert.deepEqual(bought.preparation.shop.slots.slice(1), p.shop.slots.slice(1));
  assert.equal(bought.preparation.resources.gold, p.resources.gold - offer.price);
  const free = await execute(port, bought, "refreshShop");
  assert.equal(free.preparation.resources.gold, bought.preparation.resources.gold);
  assert.equal(free.preparation.shop.freeRefreshes, 0);
  const paid = await execute(port, free, "refreshShop");
  assert.equal(paid.preparation.resources.gold, free.preparation.resources.gold - 2);
  const trained = await execute(port, paid, "buyExperience");
  assert.equal(trained.preparation.resources.gold, paid.preparation.resources.gold - 4);
  assert.equal(trained.preparation.resources.level, 9);
  assert.deepEqual(trained.preparation.shop.slots, paid.preparation.shop.slots);
});

test("trait choice keeps its effect until confirmed and supports confirming the current choice", async () => {
  const port = demoSessionPort();
  const before = value(await port.preview(input, "preparation"));
  const once = await execute(port, before, "chooseMechanism");
  assert.equal(once.preparation.mechanisms[0].choices[0].selectedOptionId, "trait0");
  assert.ok(!kinds(once).includes("chooseMechanism"));
  const opened = await execute(port, once, "openTraitChoice");
  const same = await execute(port, opened, "confirmTraitChoice");
  assert.equal(same.preparation.mechanisms[1].choices[0].selectedOptionId, "trait0");
  const reopened = await execute(port, same, "openTraitChoice");
  const draft = await execute(port, reopened, "selectTraitOption", a => a.optionId === "trait1");
  assert.equal(draft.phase.openTraitChoice.draftOptionId, "trait1");
  assert.equal(draft.preparation.mechanisms[1].choices[0].selectedOptionId, "trait0");
  const confirmed = await execute(port, draft, "confirmTraitChoice");
  assert.equal(confirmed.phase.openTraitChoice, null);
  assert.equal(confirmed.preparation.mechanisms[1].choices[0].selectedOptionId, "trait1");
});

test("tea settling grants no experience and battle retry or leave does not settle", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const port = demoSessionPort();
  let tea = value(await port.preview(input, "teaBreak"));
  tea = await execute(port, tea, "chooseTeaBreak");
  assert.equal(tea.phase.kind, "teaBreakReward");
  assert.equal(tea.latestSettlement, null);
  tea = await execute(port, tea, "claimTeaBreakReward");
  assert.equal(tea.latestSettlement.source, "teaBreak");
  assert.equal(tea.latestSettlement.experienceGained, 0);
  let battle = value(await port.preview(input, "battleUnavailable"));
  const before = battle.preparation.resources;
  const controller = new AbortController();
  const stream = value(await port.subscribeMatch({ matchId: battle.matchId }, controller.signal))[Symbol.asyncIterator]();
  assert.equal((await stream.next()).value.kind, "snapshot");
  const running = await execute(port, battle, "restartBattle");
  assert.notEqual(running.phase.attemptId, battle.phase.attemptId);
  const away = await execute(port, running, "leaveBattle");
  assert.deepEqual(kinds(away), ["move", "reenterBattle"]);
  t.mock.timers.tick(6000);
  assert.deepEqual(value(await port.getMatch(away)), away);
  const reentered = await execute(port, away, "reenterBattle");
  assert.notEqual(reentered.phase.attemptId, running.phase.attemptId);
  assert.deepEqual(reentered.preparation.resources, before);
  assert.equal(reentered.latestSettlement, null);
  t.mock.timers.tick(5000);
  const settled = value(await port.getMatch(reentered));
  assert.equal(settled.latestSettlement.source, "battle");
  assert.equal(settled.latestSettlement.experienceGained, 2);
  assert.equal(settled.revision, reentered.revision + 1);
  const events = [];
  for (let i = 0; i < 4; i++) events.push((await stream.next()).value);
  assert.deepEqual(events.map(event => event.kind), ["actionApplied", "actionApplied", "actionApplied", "battleSettled"]);
  controller.abort();
  assert.equal((await stream.next()).done, true);
  t.mock.timers.tick(5000);
  assert.equal(value(await port.getMatch(reentered)).revision, settled.revision);
});

test("model publishes its action and handing back control cancels subsequent automatic actions", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const port = demoSessionPort();
  const initial = value(await port.createMatch(input));
  const abort = new AbortController();
  const stream = value(await port.subscribeMatch(initial, abort.signal))[Symbol.asyncIterator]();
  assert.deepEqual((await stream.next()).value.snapshot, initial);
  const model = value(await port.setController({ ...initial, controller: { kind: "model", modelId: "demo" } }));
  assert.equal((await stream.next()).value.kind, "controllerChanged");
  const rejected = await port.executeAction({ matchId: model.matchId, revision: model.revision, actionId: model.legalActions[0].actionId });
  assert.equal(rejected.error.code, "controllerMismatch");
  t.mock.timers.tick(700);
  const event = (await stream.next()).value;
  assert.equal(event.kind, "actionApplied");
  assert.deepEqual(event.actor, { kind: "model", modelId: "demo" });
  assert.equal(event.action.kind, "chooseInvestmentEnvironment");
  const advanced = value(await port.getMatch(initial));
  const manual = value(await port.setController({ ...advanced, controller: { kind: "manual" } }));
  t.mock.timers.tick(2000);
  assert.deepEqual(value(await port.getMatch(initial)), manual);
  assert.equal((await stream.next()).value.kind, "controllerChanged");
  abort.abort();
  assert.equal((await stream.next()).done, true);
});

test("manual play visits all three planes in order and reaches both terminal results", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const port = demoSessionPort();
  let s = value(await port.createMatch(input));
  s = await execute(port, s, "chooseInvestmentEnvironment");
  const visits = [];
  let oldNode = "";
  for (let step = 0; step < 60 && s.phase.kind !== "finished"; step++) {
    if (s.position.nodeId !== oldNode) {
      oldNode = s.position.nodeId;
      visits.push(oldNode);
    }
    switch (s.phase.kind) {
      case "preparation":
        s = await execute(port, s, "startBattle");
        t.mock.timers.tick(5000);
        s = value(await port.getMatch(s));
        break;
      case "investmentStrategy": s = await execute(port, s, "chooseInvestmentStrategy"); break;
      case "supply": s = await execute(port, s, "takeSupply"); break;
      case "teaBreak": s = await execute(port, s, "chooseTeaBreak"); break;
      case "teaBreakReward": s = await execute(port, s, "claimTeaBreakReward"); break;
      default: assert.fail(s.phase.kind);
    }
  }
  assert.deepEqual(visits, s.route.map(entry => entry.nodeId));
  assert.equal(s.position.enteredTriggerNodes, 20);
  assert.deepEqual(s.phase, { kind: "finished", reason: "cleared" });
  assert.deepEqual(s.legalActions, []);
  const lost = value(await port.preview(input, "healthDepleted"));
  assert.deepEqual(lost.phase, { kind: "finished", reason: "healthDepleted" });
  assert.equal(lost.preparation.resources.health, 0);
});

test("developer samples open restricted views directly and hold the initial battle for inspection", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const port = demoSessionPort();
  for (const kind of ["investmentStrategy", "supply", "spotlight"]) {
    const sample = value(await port.preview(input, kind + "Preparation", true));
    assert.equal(sample.phase.kind, kind);
    assert.equal(sample.phase.view, "preparation");
    assert.deepEqual(kinds(sample), kind === "spotlight" ?
      ["move", "openCrystal", "returnToSelection"] : ["openCrystal", "returnToSelection"]);
  }
  const battle = value(await port.preview(input, "battle", true));
  t.mock.timers.tick(9000);
  assert.equal(value(await port.getMatch(battle)).phase.attemptId, battle.phase.attemptId);
  const restarted = await execute(port, battle, "restartBattle");
  t.mock.timers.tick(5000);
  assert.equal(value(await port.getMatch(restarted)).latestSettlement.source, "battle");
  port.dispose();
  assert.equal((await port.getMatch(restarted)).error.code, "matchNotFound");
});

test("feedback records observations for this revision and keeps simulated resources intact", async () => {
  const port = demoSessionPort();
  const snapshot = value(await port.preview(input, "preparation"));
  const next = await execute(port, snapshot, "lockShop");
  const feedback = { matchId: snapshot.matchId, revision: snapshot.revision, action: "hold", observed: { gold: 0, health: 64 } };
  assert.equal((await port.saveSessionFeedback(feedback)).error.code, "revisionConflict");
  assert.equal((await port.saveSessionFeedback({ ...feedback, revision: next.revision })).ok, true);
  assert.equal(value(await port.matchInsights(next)).lastAction, "hold");
  assert.deepEqual(value(await port.getMatch(next)), next);
});

test("HTTP sends only the revision-scoped action and streams updates across UTF-8 chunks", async t => {
  const action = { matchId: "sample&one", revision: 7, actionId: "equip-9" };
  const update = { kind: "battleUnavailable", matchId: action.matchId, revision: 8, message: "失败" };
  const encoder = new TextEncoder();
  const bytes = encoder.encode(JSON.stringify(update) + "\n" + JSON.stringify(update));
  const split = bytes.findIndex(byte => byte > 127) + 1;
  t.mock.method(globalThis, "fetch", async (url, init) => {
    if (url.endsWith("/match/actions")) {
      assert.equal(init.method, "POST");
      assert.deepEqual(JSON.parse(init.body), action);
      return Response.json({ ok: false, error: { code: "revisionConflict", currentRevision: 8 } }, { status: 409 });
    }
    assert.equal(url, "https://example.test/v1/match/updates?matchId=sample%26one");
    return new Response(new ReadableStream({ start(controller) {
      controller.enqueue(bytes.slice(0, split));
      controller.enqueue(bytes.slice(split));
      controller.close();
    } }), { headers: { "Content-Type": "application/x-ndjson" } });
  });
  const port = httpSessionPort("https://example.test/v1/");
  assert.equal((await port.executeAction(action)).error.code, "revisionConflict");
  const updates = [];
  for await (const event of value(await port.subscribeMatch(action))) updates.push(event);
  assert.deepEqual(updates, [update, update]);
});
