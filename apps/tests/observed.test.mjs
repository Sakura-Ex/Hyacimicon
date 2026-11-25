import { test } from "node:test";
import assert from "node:assert/strict";
import { appendObservation, decodeHistory, emptyObservation, validateObservation } from "../src/observed-match.ts";

function observation() {
  const value = emptyObservation();
  Object.assign(value.fields, { match: "实际对局 A", node: "1-1", phase: "备战", health: "100", maxHealth: "100", gold: "8" });
  return value;
}

test("记录实际状态，不产生模拟报价或战斗结果", () => {
  const first = observation();
  first.shop[0] = "角色 A · 1 金币";
  const h1 = appendObservation([], first, "2026-11-25T00:00:00Z");
  first.shop[0] = "角色 B · 2 金币";
  const h2 = appendObservation(h1, first, "2026-11-25T00:01:00Z");
  assert.equal(h2[0].observation.shop[0], "角色 A · 1 金币");
  assert.equal(h2[1].observation.shop[0], "角色 B · 2 金币");
  assert.equal(h2[1].observation.fields.settlement, "");
  assert.deepEqual(decodeHistory(JSON.stringify(h2)), h2);
});

test("未知信息保持空字符串，错误值及损坏历史明确拒绝", () => {
  const value = observation();
  value.fields.gold = "";
  assert.equal(validateObservation(value), undefined);
  value.fields.health = "101";
  assert.match(validateObservation(value), /血量/);
  value.fields.health = "100";
  value.fields.gold = "-1";
  assert.throws(() => appendObservation([], value, "now"), /金币/);
  assert.throws(() => decodeHistory('[{"revision":1,"observation":{}}]'), /格式/);
  assert.deepEqual(decodeHistory(null), []);
});
