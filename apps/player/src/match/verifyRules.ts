import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildCatalogFromRaw } from '../data/encyclopediaLoader.ts';
import { LocalSimulatedMatchService } from './localMatchService.ts';
import {
  calculateBattleHealthSettlement,
  calculateStreakGold,
  isZoneEligible,
} from './rules.ts';

function readDataJson(dataDir: string, fileName: string): unknown {
  const fullPath = path.join(dataDir, 'data', 'encyclopedia', fileName);
  const parsed: unknown = JSON.parse(fs.readFileSync(fullPath, 'utf8'));
  return parsed;
}

async function main(): Promise<void> {
  const envDir = process.env.HSR_CURRENCY_WARS_DATA ?? process.env.HSR_DATA_DIR;
  const dataDir = envDir
    ? path.resolve(envDir)
    : path.resolve(import.meta.dirname, '../../../../../hsr-currency-wars-data');

  const catalog = buildCatalogFromRaw({
    characters: readDataJson(dataDir, 'characters.json'),
    items: readDataJson(dataDir, 'items.json'),
    itemDetails: readDataJson(dataDir, 'item_details.json'),
    traits: readDataJson(dataDir, 'traits.json'),
    traitDetails: readDataJson(dataDir, 'trait_details.json'),
    strategies: readDataJson(dataDir, 'strategies.json'),
    environments: readDataJson(dataDir, 'environments.json'),
    auras: readDataJson(dataDir, 'auras.json'),
    affixes: readDataJson(dataDir, 'affixes.json'),
    rivals: readDataJson(dataDir, 'rivals.json'),
    companions: readDataJson(dataDir, 'companions.json'),
    presentation: readDataJson(dataDir, 'presentation.json'),
  });

  // 1. Battle Health Settlement Rules (1-8, 1-9, 2-6, 2-7, encounter <25%, boss <33%, maxHealth cap, 0 HP)
  const s18 = calculateBattleHealthSettlement({
    nodeId: '1-8',
    kind: 'reward',
    actualProgress: 50,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(s18.baseHealthLost, 5);
  assert.equal(s18.progressHealthLost, 5);
  assert.equal(s18.healthAfter, 92);

  const s19BossLow = calculateBattleHealthSettlement({
    nodeId: '1-9',
    kind: 'boss',
    actualProgress: 32,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(s19BossLow.baseHealthLost, 10);
  // 10 * (100 - 32)/100 = 6.8 -> rounded 7 -> doubled (<33%) = 14
  assert.equal(s19BossLow.progressHealthLost, 14);
  assert.equal(s19BossLow.healthAfter, 78);

  const s27Cap = calculateBattleHealthSettlement({
    nodeId: '2-7',
    kind: 'boss',
    actualProgress: 100,
    healthBefore: 99,
    maxHealth: 100,
  });
  assert.equal(s27Cap.healthLost, 0);
  assert.equal(s27Cap.healthAfter, 100);

  const sDepleted = calculateBattleHealthSettlement({
    nodeId: '3-7',
    kind: 'boss',
    actualProgress: 0,
    healthBefore: 20,
    maxHealth: 100,
  });
  assert.equal(sDepleted.healthAfter, 0);

  // Encounter < 25% doubles rounded progress loss, >= 25% does not
  const sEncLow = calculateBattleHealthSettlement({
    nodeId: '1-7',
    kind: 'encounter',
    actualProgress: 24,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(sEncLow.baseHealthLost, 5);
  assert.equal(sEncLow.progressHealthLost, 16); // round(10*0.76)=8 -> doubled=16

  const sEncPass = calculateBattleHealthSettlement({
    nodeId: '1-7',
    kind: 'encounter',
    actualProgress: 25,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(sEncPass.progressHealthLost, 8);

  // 2-6 vs 2-7 base health loss switch (10 vs 15)
  const s26 = calculateBattleHealthSettlement({
    nodeId: '2-6',
    kind: 'reward',
    actualProgress: 50,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(s26.baseHealthLost, 10);

  const s27Fail = calculateBattleHealthSettlement({
    nodeId: '2-7',
    kind: 'boss',
    actualProgress: 50,
    healthBefore: 100,
    maxHealth: 100,
  });
  assert.equal(s27Fail.baseHealthLost, 15);

  assert.equal(calculateStreakGold(1), 0);
  assert.equal(calculateStreakGold(2), 1);
  assert.equal(calculateStreakGold(4), 2);
  assert.equal(calculateStreakGold(6), 3);

  assert.equal(isZoneEligible('前台', 'back'), false);
  assert.equal(isZoneEligible('后台', 'front'), false);
  assert.equal(isZoneEligible('前后台', 'front'), true);
  assert.equal(isZoneEligible('前后台', 'back'), true);

  // 2. MatchService validation & deterministic flow
  const svc = new LocalSimulatedMatchService(catalog);

  const badDiff = await svc.createMatch({
    accountId: 'trial-all',
    rulesVersion: 's2-v4.7',
    enemyDifficulty: 300,
    affixCount: 2,
    randomness: { kind: 'seeded', seed: 'test-seed' },
  });
  assert.equal(badDiff.ok, false);
  if (!badDiff.ok) {
    assert.equal(badDiff.error.code, 'invalidInput');
  }

  const zeroAffix = await svc.createMatch({
    accountId: 'trial-all',
    rulesVersion: 's2-v4.7',
    enemyDifficulty: 120,
    affixCount: 0,
    randomness: { kind: 'seeded', seed: 'test-seed' },
  });
  assert.equal(zeroAffix.ok, true);
  if (zeroAffix.ok) {
    assert.equal(zeroAffix.value.affixes.length, 0);
  }

  const created = await svc.createMatch({
    accountId: 'trial-all',
    rulesVersion: 's2-v4.7',
    enemyDifficulty: 120,
    affixCount: 3,
    randomness: { kind: 'seeded', seed: 'test-seed' },
  });
  assert.equal(created.ok, true);
  if (!created.ok) {
    throw new Error('Expected createMatch to succeed');
  }

  let snap = created.value;
  assert.equal(snap.affixes.length, 3);
  assert.ok(snap.affixes[0]?.name.includes('强化'));
  assert.equal(snap.phase.kind, 'investmentEnvironment');

  // Choose investment environment -> enters 1-1
  const chooseEnv = snap.legalActions.find(
    (a) => a.action.kind === 'chooseInvestmentEnvironment',
  );
  assert.ok(chooseEnv);
  const res1 = await svc.executeAction({
    matchId: snap.matchId,
    revision: snap.revision,
    actionId: chooseEnv.actionId,
  });
  assert.equal(res1.ok, true);
  if (!res1.ok) {
    throw new Error('Expected chooseInvestmentEnvironment to succeed');
  }
  snap = res1.value;
  assert.equal(snap.phase.kind, 'preparation');
  assert.equal(snap.position?.nodeId, '1-1');
  assert.equal(snap.preparation?.resources.level, 3);
  assert.equal(snap.preparation?.resources.gold, 3);
  assert.equal(snap.preparation?.crystals.length, 2);

  // Stale revision conflict check
  const firstAct = snap.legalActions[0];
  assert.ok(firstAct);
  const stale = await svc.executeAction({
    matchId: snap.matchId,
    revision: 0,
    actionId: firstAct.actionId,
  });
  assert.equal(stale.ok, false);
  if (!stale.ok) {
    assert.equal(stale.error.code, 'revisionConflict');
  }

  // Controller handoff check
  const handoff = await svc.setController({
    matchId: snap.matchId,
    revision: snap.revision,
    controller: { kind: 'model', modelId: 'test-model' },
  });
  assert.equal(handoff.ok, true);
  if (!handoff.ok) {
    throw new Error('Expected setController to succeed');
  }
  const handoffFirstAct = handoff.value.legalActions[0];
  assert.ok(handoffFirstAct);
  const wrongCaller = await svc.executeAction(
    {
      matchId: handoff.value.matchId,
      revision: handoff.value.revision,
      actionId: handoffFirstAct.actionId,
    },
    { kind: 'manual' },
  );
  assert.equal(wrongCaller.ok, false);
  if (!wrongCaller.ok) {
    assert.equal(wrongCaller.error.code, 'controllerMismatch');
  }

  // Different modelId must also trigger controllerMismatch
  const wrongModel = await svc.executeAction(
    {
      matchId: handoff.value.matchId,
      revision: handoff.value.revision,
      actionId: handoffFirstAct.actionId,
    },
    { kind: 'model', modelId: 'other-model' },
  );
  assert.equal(wrongModel.ok, false);
  if (!wrongModel.ok) {
    assert.equal(wrongModel.error.code, 'controllerMismatch');
  }

  // Hand back to manual and verify battle restart (same seed) vs leave -> reenter (new seed)
  const backManual = await svc.setController({
    matchId: handoff.value.matchId,
    revision: handoff.value.revision,
    controller: { kind: 'manual' },
  });
  assert.equal(backManual.ok, true);
  if (!backManual.ok) {
    throw new Error('Expected manual handback to succeed');
  }
  snap = backManual.value;

  const startBattleAct = snap.legalActions.find((a) => a.action.kind === 'startBattle');
  assert.ok(startBattleAct);
  const battleRes = await svc.executeAction({
    matchId: snap.matchId,
    revision: snap.revision,
    actionId: startBattleAct.actionId,
  });
  assert.equal(battleRes.ok, true);
  if (!battleRes.ok) {
    throw new Error('Expected startBattle to succeed');
  }
  snap = battleRes.value;
  assert.equal(snap.phase.kind, 'battle');
  const firstSeed = svc.getDebugInspect(snap.matchId)?.activeBattleSeed;
  assert.ok(firstSeed);

  const restartAct = snap.legalActions.find((a) => a.action.kind === 'restartBattle');
  assert.ok(restartAct);
  const restarted = await svc.executeAction({
    matchId: snap.matchId,
    revision: snap.revision,
    actionId: restartAct.actionId,
  });
  assert.equal(restarted.ok, true);
  if (!restarted.ok) {
    throw new Error('Expected restartBattle to succeed');
  }
  assert.equal(svc.getDebugInspect(restarted.value.matchId)?.activeBattleSeed, firstSeed);

  const leaveAct = restarted.value.legalActions.find((a) => a.action.kind === 'leaveBattle');
  assert.ok(leaveAct);
  const left = await svc.executeAction({
    matchId: restarted.value.matchId,
    revision: restarted.value.revision,
    actionId: leaveAct.actionId,
  });
  assert.equal(left.ok, true);
  if (!left.ok) {
    throw new Error('Expected leaveBattle to succeed');
  }
  assert.equal(left.value.phase.kind, 'battleAway');

  const reenterAct = left.value.legalActions.find((a) => a.action.kind === 'reenterBattle');
  assert.ok(reenterAct);
  const reentered = await svc.executeAction({
    matchId: left.value.matchId,
    revision: left.value.revision,
    actionId: reenterAct.actionId,
  });
  assert.equal(reentered.ok, true);
  if (!reentered.ok) {
    throw new Error('Expected reenterBattle to succeed');
  }
  assert.notEqual(svc.getDebugInspect(reentered.value.matchId)?.activeBattleSeed, firstSeed);

  console.log('ALL CURRENCY WARS RULE & CONTRACT CHECKS PASSED!');
}

void main();
