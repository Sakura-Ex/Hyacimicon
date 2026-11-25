import type { PositionKind } from '../types/hsr';
import type { NodeId, NodeKind, Plane, RouteNode, Slot } from './contract';

export const FIRST_ROUTE_NODE: RouteNode = { nodeId: '1-1', kind: 'reward' };

export const CANONICAL_ROUTE: readonly RouteNode[] = [
  FIRST_ROUTE_NODE,
  { nodeId: '1-2', kind: 'reward' },
  { nodeId: '1-3', kind: 'battle' },
  { nodeId: '1-4', kind: 'battle' },
  { nodeId: '1-5', kind: 'supply' },
  { nodeId: '1-6', kind: 'battle' },
  { nodeId: '1-7', kind: 'encounter' },
  { nodeId: '1-8', kind: 'reward' },
  { nodeId: '1-9', kind: 'boss' },
  { nodeId: '2-1', kind: 'battle' },
  { nodeId: '2-2', kind: 'battle' },
  { nodeId: '2-3', kind: 'supply' },
  { nodeId: '2-4', kind: 'battle' },
  { nodeId: '2-5', kind: 'encounter' },
  { nodeId: '2-6', kind: 'reward' },
  { nodeId: '2-7', kind: 'boss' },
  { nodeId: '3-1', kind: 'battle' },
  { nodeId: '3-2', kind: 'battle' },
  { nodeId: '3-3', kind: 'supply' },
  { nodeId: '3-4', kind: 'battle' },
  { nodeId: '3-5', kind: 'encounter' },
  { nodeId: '3-6', kind: 'reward' },
  { nodeId: '3-7', kind: 'boss' },
];

export const INVESTMENT_STRATEGY_NODES: ReadonlySet<NodeId> = new Set<NodeId>([
  '1-3',
  '2-2',
  '3-2',
]);

export const SPOTLIGHT_BOSS_NODES: ReadonlySet<NodeId> = new Set<NodeId>(['1-9', '2-7']);

export const SHOP_EXP_TO_NEXT: Record<number, number | null> = {
  1: 0,
  2: 0,
  3: 4,
  4: 6,
  5: 20,
  6: 40,
  7: 52,
  8: 72,
  9: 84,
  10: null,
};

export interface ShopCostOdds {
  readonly 1: number;
  readonly 2: number;
  readonly 3: number;
  readonly 4: number;
  readonly 5: number;
}

export const SHOP_ODDS_BY_LEVEL: Record<number, ShopCostOdds> = {
  1: { 1: 100, 2: 0, 3: 0, 4: 0, 5: 0 },
  2: { 1: 100, 2: 0, 3: 0, 4: 0, 5: 0 },
  3: { 1: 100, 2: 0, 3: 0, 4: 0, 5: 0 },
  4: { 1: 65, 2: 25, 3: 10, 4: 0, 5: 0 },
  5: { 1: 45, 2: 33, 3: 20, 4: 2, 5: 0 },
  6: { 1: 30, 2: 40, 3: 25, 4: 5, 5: 0 },
  7: { 1: 19, 2: 30, 3: 40, 4: 10, 5: 1 },
  8: { 1: 18, 2: 25, 3: 32, 4: 22, 5: 3 },
  9: { 1: 15, 2: 20, 3: 25, 4: 30, 5: 10 },
  10: { 1: 5, 2: 10, 3: 20, 4: 40, 5: 25 },
};

const DEFAULT_ODDS: ShopCostOdds = { 1: 100, 2: 0, 3: 0, 4: 0, 5: 0 };

export function getShopOdds(level: number): ShopCostOdds {
  return SHOP_ODDS_BY_LEVEL[level] ?? DEFAULT_ODDS;
}

export const POOL_COPIES_BY_COST: Record<number, number> = {
  1: 30,
  2: 25,
  3: 18,
  4: 10,
  5: 9,
};

export function planeOfNode(nodeId: NodeId): Plane {
  if (nodeId.startsWith('1-')) {
    return 1;
  }
  if (nodeId.startsWith('2-')) {
    return 2;
  }
  return 3;
}

export function baseHealthLossForNode(nodeId: NodeId): number {
  const [planeStr, stepStr] = nodeId.split('-');
  const plane = Number(planeStr);
  const step = Number(stepStr);
  if (plane === 1 && step <= 8) {
    return 5;
  }
  if ((plane === 1 && step === 9) || (plane === 2 && step <= 6)) {
    return 10;
  }
  return 15;
}

export interface BattleHealthCalcInput {
  nodeId: NodeId;
  kind: NodeKind;
  actualProgress: number;
  healthBefore: number;
  maxHealth: number;
}

export interface BattleHealthCalcOutput {
  baseHealthLost: number;
  progressHealthLost: number;
  healthLost: number;
  fixedRecovery: number;
  healthAfter: number;
}

export function calculateBattleHealthSettlement(
  input: BattleHealthCalcInput,
): BattleHealthCalcOutput {
  const fixedRecovery = 2;
  if (input.actualProgress >= 100) {
    return {
      baseHealthLost: 0,
      progressHealthLost: 0,
      healthLost: 0,
      fixedRecovery,
      healthAfter: Math.min(input.maxHealth, input.healthBefore + fixedRecovery),
    };
  }

  const clamped = Math.max(0, Math.min(99.999, input.actualProgress));
  const baseHealthLost = baseHealthLossForNode(input.nodeId);
  const plane = planeOfNode(input.nodeId);
  const planeCoeff = plane === 1 ? 10 : plane === 2 ? 15 : 20;

  let progressHealthLost = Math.round((planeCoeff * (100 - clamped)) / 100);
  if (
    (input.kind === 'encounter' && clamped < 25) ||
    (input.kind === 'boss' && clamped < 33)
  ) {
    progressHealthLost *= 2;
  }

  const healthLost = baseHealthLost + progressHealthLost;
  const rawAfter = input.healthBefore - healthLost + fixedRecovery;
  const healthAfter = Math.max(0, Math.min(input.maxHealth, rawAfter));

  return {
    baseHealthLost,
    progressHealthLost,
    healthLost,
    fixedRecovery,
    healthAfter,
  };
}

export function calculateStreakGold(streak: number): number {
  if (streak >= 6) {
    return 3;
  }
  if (streak >= 4) {
    return 2;
  }
  if (streak >= 2) {
    return 1;
  }
  return 0;
}

export function isZoneEligible(
  position: PositionKind,
  zone: Slot['zone'],
): boolean {
  if (zone === 'bench') {
    return true;
  }
  if (zone === 'front') {
    return position === '前台' || position === '前后台';
  }
  return position === '后台' || position === '前后台';
}

export function pieceCopiesCount(stars: number): number {
  if (stars <= 1) {
    return 1;
  }
  if (stars === 2) {
    return 3;
  }
  if (stars === 3) {
    return 9;
  }
  return 27;
}

export function sellGoldValue(cost: number, stars: number): number {
  const copies = pieceCopiesCount(stars);
  if (stars <= 1) {
    return cost;
  }
  return Math.max(1, cost * copies - 1);
}
