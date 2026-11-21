import assert from "node:assert/strict";
import test from "node:test";
import { demoPort } from "../src/demo.ts";

test("a confirmed observation changes only the recorded fields and expires the old revision", async () => {
  const port = demoPort();
  const [account] = await port.accounts();
  const initial = await port.match(account.id);
  assert.equal(initial.gold, null);
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
  });
  assert.equal(saved.gold, 0);
  assert.equal(saved.lastAction, "hold");
  assert.equal(saved.revision, locked.revision + 1);
  assert.equal(saved.locked, true);
  assert.deepEqual(saved.bench, initial.bench);
  assert.equal(initial.gold, null);
});
test("preview frames reference their actor and carry no invented effects or events", async () => {
  const port = demoPort();
  const [account] = await port.accounts();
  const match = await port.match(account.id);
  const frames = await port.battle(match.id);
  assert.ok(frames.length >= 2);
  for (const frame of frames) {
    assert.ok(frame.units.some((unit) => unit.id === frame.actorId));
    assert.deepEqual(frame.log, []);
    assert.deepEqual(frame.targets, []);
  }
  await assert.rejects(port.battle("unknown"), /找不到战斗记录/);
});
