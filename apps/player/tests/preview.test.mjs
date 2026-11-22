import assert from "node:assert/strict";
import test from "node:test";
import { demoPort } from "../src/demo.ts";

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
  const locked = await port.setShopLock(initial.id, initial.revision, true);
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
  await port.setShopLock(match.id, match.revision, true);
  await assert.rejects(port.battle(source), /版本已更新/);
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
