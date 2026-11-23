import assert from "node:assert/strict";
import test from "node:test";
import { demoPort } from "../src/demo.ts";
import { httpMatchPort } from "../src/match-port.ts";
import { commandsForDrop } from "../src/match-drag.ts";

function execute(port, match, kind, predicate = () => true) {
  const command = match.commands.find(({ operation }) => operation.kind === kind && predicate(operation));
  assert.ok(command, `${kind} must be available`);
  return port.executeMatchCommand({ matchId: match.id, revision: match.revision, commandId: command.id });
}

test("each preview character references its own full portrait", async () => {
  const port = demoPort();
  const { accounts: [account] } = await port.accounts();
  const { allies } = await port.battleCatalogue();
  assert.equal(account.characters.find(c => c.id === "1310").portrait, "/portraits/1310.png");
  for (const character of [...account.characters, ...allies]) {
    assert.equal(character.portrait, `/portraits/${character.id}.png`);
  }
});

test("a confirmed observation changes only the recorded fields and expires the old revision", async () => {
  const port = demoPort();
  const { accounts: [account] } = await port.accounts();
  const initial = await port.match(account.id);
  assert.equal(initial.gold, 37);
  assert.deepEqual(initial.advice, []);
  const locked = await execute(port, initial, "lockShop");
  const input = {
    matchId: initial.id,
    revision: initial.revision,
    action: "hold",
    observed: { gold: 0 },
  };
  await assert.rejects(port.saveFeedback(input), /版本已更新/);
  await assert.rejects(
    port.saveFeedback({
      ...input,
      revision: locked.revision,
      observed: { gold: -1 },
    }),
    /有效的金币/,
  );
  const saved = await port.saveFeedback({
    ...input,
    revision: locked.revision,
    observed: { gold: 0, health: 64 },
  });
  assert.equal(saved.gold, 0);
  assert.equal(saved.lastAction, "hold");
  assert.equal(saved.revision, locked.revision + 1);
  assert.equal(saved.locked, true);
  assert.deepEqual(saved.bench, initial.bench);
  assert.equal(saved.health, 64);
  assert.equal(saved.node, initial.node);
  assert.deepEqual(saved.planes, initial.planes);
  assert.equal(initial.gold, 37);
});
test("the deployment preview shows a route, run health, stars and equipment in all rows", async () => {
  const match = await demoPort().match("preview");
  const steps = match.planes.flat();
  assert.equal(steps.length, 20);
  assert.equal(steps[9].id, match.node);
  assert.equal(match.maxHealth, 100);
  for (const row of [match.front, match.back, match.bench]) {
    assert.ok(row.every(piece => piece.stars > 0));
    assert.ok(row.some(piece => piece.gear.length > 0));
  }
});
test("currency preview contains deployed units and carries no fabricated action frames", async () => {
  const port = demoPort();
  const { accounts: [account] } = await port.accounts();
  const match = await port.match(account.id);
  const source = {
    mode: "currencyWars",
    matchId: match.id,
    revision: match.revision,
  };
  const result = await port.battle(source);
  assert.equal(result.kind, "preview");
  assert.equal(
    result.units.filter((u) => u.side === "后台").length,
    match.back.length,
  );
  assert.deepEqual(result.units[0].gear, match.front[0].gear);
  await execute(port, match, "lockShop");
  await assert.rejects(port.battle(source), /版本已更新/);
});

test("equipment uses the instance ID when the same character appears twice", async () => {
  const port = demoPort();
  const before = await port.match("preview");
  const [item] = before.inventory;
  const duplicate = before.bench.find(piece => piece.id === "1204");
  const deployed = before.front.find(piece => piece.id === duplicate.id);
  const equip = operation => operation.itemId === item.itemId && operation.pieceId === duplicate.pieceId;
  const equipped = await execute(port, before, "equip", equip);
  assert.deepEqual(equipped.front.find(piece => piece.pieceId === deployed.pieceId), deployed);
  assert.ok(equipped.bench.find(piece => piece.pieceId === duplicate.pieceId).gear.some(gear => gear.itemId === item.itemId));
  assert.ok(!equipped.inventory.some(gear => gear.itemId === item.itemId));
  await assert.rejects(execute(port, before, "equip", equip), /版本已更新/);
  const removed = await execute(port, equipped, "useItem", operation => operation.pieceId === duplicate.pieceId);
  assert.ok(removed.inventory.some(gear => gear.itemId === item.itemId));
  assert.equal(removed.bench.find(piece => piece.pieceId === duplicate.pieceId).gear.length, 0);
});

test("drag targets resolve by instance and only execute commands offered by the current snapshot", async () => {
  const port = demoPort("stock");
  const match = await port.match("preview");
  assert.equal(match.inventory.length, 30);
  assert.equal(new Set(match.inventory.map(item => item.itemId)).size, 30);
  const item = match.inventory.at(-1);
  const piece = match.bench.at(-1);
  const [equip] = commandsForDrop(match.commands, item.itemId, { pieceId: piece.pieceId });
  assert.equal(equip.operation.kind, "equip");
  assert.deepEqual(commandsForDrop(match.commands, item.itemId, { pieceId: "missing" }), []);
  const next = await port.executeMatchCommand({ matchId: match.id, revision: match.revision, commandId: equip.id });
  assert.equal(next.inventory.length, 29);
  assert.equal(next.bench.at(-1).gear[0].itemId, item.itemId);
});

test("drag separates equipment pairs, item-targeted consumables and character-targeted consumables", () => {
  const commands = [
    { id: "merge", operation: { kind: "combine", itemId: "one", otherItemId: "two" } },
    { id: "extract", operation: { kind: "useItem", itemId: "tool", targetItemId: "two", pieceId: "a" } },
    { id: "wrench", operation: { kind: "useItem", itemId: "tool", pieceId: "b" } },
    { id: "use", operation: { kind: "useItem", itemId: "tool" } },
  ];
  assert.deepEqual(commandsForDrop(commands, "two", { itemId: "one" }).map(c => c.id), ["merge"]);
  assert.deepEqual(commandsForDrop(commands, "tool", { itemId: "two", pieceId: "a" }).map(c => c.id), ["extract"]);
  assert.deepEqual(commandsForDrop(commands, "tool", { pieceId: "b" }).map(c => c.id), ["wrench"]);
  assert.deepEqual(commandsForDrop(commands, "tool", { pieceId: "a" }), []);
  assert.deepEqual(commandsForDrop(commands, "one", { itemId: "one" }), []);
});

test("the full roster preview fills nine uniquely addressable slots in both rows and has a long trait list", async () => {
  const match = await demoPort("full").match("preview");
  assert.equal(match.backCapacity, 9);
  assert.equal(match.benchCapacity, 9);
  for (const zone of [match.back, match.bench]) {
    assert.deepEqual(zone.map(piece => piece.slot), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert.equal(new Set(zone.map(piece => piece.pieceId)).size, 9);
  }
  assert.ok(match.traits.length > 12);
  assert.ok(match.traits.every(trait => trait.progress?.thresholds.length));
});

test("shop actions use the offer ID and the new snapshot carries the resulting price and balance", async () => {
  const port = demoPort();
  const before = await port.match("preview");
  const offer = before.offers[0];
  assert.deepEqual(offer.traits.map(trait => trait.name), ["云上五骁", "召唤"]);
  const bought = await execute(port, before, "buy", operation => operation.offerId === offer.offerId);
  assert.ok(!bought.offers.some(entry => entry.offerId === offer.offerId));
  assert.equal(bought.gold, before.gold - offer.cost);
  assert.equal(bought.bench.at(-1).id, offer.unit.id);
  const refreshed = await execute(port, bought, "refreshShop");
  assert.equal(refreshed.gold, bought.gold - 2);
  assert.equal(refreshed.offers.length, 5);
  assert.ok(refreshed.offers.every(entry => entry.traits.length >= 2 && entry.traits.every(trait => trait.image.startsWith("/traits/"))));
});

test("buying experience updates the team level and balance through the command port", async () => {
  const port = demoPort();
  const before = await port.match("preview");
  const next = await execute(port, before, "buyExperience");
  assert.equal(next.gold, before.gold - 4);
  assert.equal(next.experience.level, before.experience.level + 1);
  assert.equal(next.capacity, before.capacity + 1);
  assert.deepEqual(next.offers, before.offers);
});

test("aura accepts deployed instances and tea break resolves its own reward", async () => {
  const auraPort = demoPort("aura");
  const aura = await auraPort.match("preview");
  const candidates = aura.commands.filter(command => command.operation.kind === "chooseAura");
  const deployed = new Set([...aura.front, ...aura.back].map(piece => piece.pieceId));
  assert.ok(candidates.every(({ operation }) => deployed.has(operation.pieceId)));
  const assigned = await execute(auraPort, aura, "chooseAura");
  assert.equal(assigned.aura.pieceId, candidates[0].operation.pieceId);
  assert.equal(assigned.event, undefined);

  const teaPort = demoPort("teaBreak");
  const tea = await teaPort.match("preview");
  const rewarded = await execute(teaPort, tea, "teaBreak");
  assert.equal(rewarded.consumables.length, tea.consumables.length + 1);
  assert.equal(rewarded.event, undefined);
});

test("the HTTP port sends only the revision-scoped command and reports version conflicts", async (t) => {
  const input = { matchId: "sample", revision: 7, commandId: "equip-9" };
  const snapshot = { id: input.matchId, revision: 8 };
  let requestCount = 0;
  t.mock.method(globalThis, "fetch", async (url, init) => {
    if (++requestCount === 1) {
      assert.equal(url, "https://example.test/v1/match?accountId=a%26b");
      assert.equal(init.cache, "no-store");
    } else {
      assert.equal(url, "https://example.test/v1/match/commands");
      assert.equal(init.method, "POST");
      assert.deepEqual(JSON.parse(init.body), input);
    }
    return requestCount === 3 ? new Response(null, { status: 409 }) : Response.json(snapshot);
  });
  const port = httpMatchPort("https://example.test/v1/");
  assert.deepEqual(await port.match("a&b"), snapshot);
  assert.deepEqual(await port.executeMatchCommand(input), snapshot);
  await assert.rejects(port.executeMatchCommand(input), /重新读取局面/);
  assert.equal(requestCount, 3);
});
test("standard combat uses its selected lineup independently of a match", async () => {
  const port = demoPort();
  const { initial, enemies } = await port.battleCatalogue();
  const input = {
    ...initial,
    allyIds: ["1409"],
    enemyIds: enemies.map((u) => u.id),
  };
  const frame = await port.battle(input);
  assert.equal(frame.kind, "preview");
  assert.deepEqual(
    frame.units.map((u) => u.id),
    [...input.allyIds, ...input.enemyIds],
  );
  assert.ok(frame.units.every((u) => !u.gear.length));
  await assert.rejects(port.battle({ ...input, allyIds: [] }), /选择参战单位/);
  await assert.rejects(
    port.battle({ ...input, enemyIds: ["invalid"] }),
    /未知参战单位/,
  );
});
