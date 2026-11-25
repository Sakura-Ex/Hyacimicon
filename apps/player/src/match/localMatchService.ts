import type { EncyclopediaCatalog } from '../data/encyclopediaLoader';
import type { AiRecommendation } from '../types/hsr';
import type {
  BattleCompletion,
  Controller,
  CreateMatchInput,
  CrystalQuality,
  CrystalStack,
  ExecuteActionInput,
  InvestmentQuality,
  ItemStack,
  LegalAction,
  MatchAction,
  MatchDebugInspect,
  MatchError,
  MatchLogEntry,
  MatchPhase,
  MatchRef,
  MatchResult,
  MatchService,
  MatchSnapshot,
  MatchUpdate,
  NodeId,
  NodeKind,
  NodeSettlement,
  OpponentPreview,
  Piece,
  Plane,
  Preparation,
  RefreshableChoice,
  RuleEntry,
  SetControllerInput,
  ShopOffer,
  Slot,
  SupplyOption,
  TraitProgress,
} from './contract';
import { SeededRng } from './rng';
import {
  CANONICAL_ROUTE,
  FIRST_ROUTE_NODE,
  INVESTMENT_STRATEGY_NODES,
  POOL_COPIES_BY_COST,
  SHOP_EXP_TO_NEXT,
  SPOTLIGHT_BOSS_NODES,
  calculateBattleHealthSettlement,
  calculateStreakGold,
  getShopOdds,
  isZoneEligible,
  pieceCopiesCount,
  planeOfNode,
  sellGoldValue,
} from './rules';

interface InternalMatchState {
  matchId: string;
  revision: number;
  rulesVersion: string;
  accountId: string;
  accountConfigVersion: string;
  enemyDifficulty: number;
  seed: string;
  rng: SeededRng;
  controller: Controller;
  routeIndex: number;
  enteredTriggerNodes: number;
  phase: MatchPhase;
  rival: RuleEntry | null;
  opponents: OpponentPreview[];
  affixes: RuleEntry[];
  environment: RuleEntry | null;
  companion: RuleEntry | null;
  preparation: Preparation | null;
  latestSettlement: NodeSettlement | null;
  poolCounts: Record<string, number>;
  nextIdCounter: number;
  activeBattleSeed: string | null;
  pendingStrategyAfterBattle: boolean;
  /** Tracks whether the current node has already had its strategy chosen. */
  strategySelectedForIndex: Record<number, boolean>;
  /** Tracks whether spotlight has been selected for the current boss node. */
  spotlightSelectedForIndex: Record<number, boolean>;
  actionLog: MatchLogEntry[];
  subscribers: Set<(update: MatchUpdate) => void>;
}

const CRYSTAL_LABELS: Record<CrystalQuality, string> = {
  white: '白色晶矿',
  blue: '蓝色晶矿',
  gold: '黄金晶矿',
  prismatic: '棱彩晶矿',
};

function ok<T>(value: T): MatchResult<T> {
  return { ok: true, value };
}

function fail<T>(error: MatchError): MatchResult<T> {
  return { ok: false, error };
}

function cloneSnapshotData<T>(value: T): T {
  return structuredClone(value);
}

export class LocalSimulatedMatchService implements MatchService {
  private readonly catalog: EncyclopediaCatalog;
  private readonly matches = new Map<string, InternalMatchState>();
  private matchCounter = 1;

  public constructor(catalog: EncyclopediaCatalog) {
    this.catalog = catalog;
  }

  public async createMatch(
    input: CreateMatchInput,
  ): Promise<MatchResult<MatchSnapshot>> {
    if (
      !Number.isInteger(input.enemyDifficulty) ||
      input.enemyDifficulty < 0 ||
      input.enemyDifficulty >= 300
    ) {
      return fail({
        code: 'invalidInput',
        message: '敌人难度必须是小于 300 的非负整数',
      });
    }

    if (
      !Number.isInteger(input.affixCount) ||
      input.affixCount < 0 ||
      input.affixCount > 4
    ) {
      return fail({
        code: 'invalidInput',
        message: '词缀数量必须是 0 到 4 的整数',
      });
    }

    const seed =
      input.randomness.kind === 'seeded' && input.randomness.seed.trim() !== ''
        ? input.randomness.seed.trim()
        : `cw-${Date.now().toString(36)}-${this.matchCounter}`;

    const rng = new SeededRng(seed);
    const matchId = `match-${this.matchCounter}`;
    this.matchCounter += 1;

    const poolCounts: Record<string, number> = {};
    for (const c of this.catalog.characters) {
      const cap = POOL_COPIES_BY_COST[c.cost];
      if (cap !== undefined) {
        poolCounts[c.id] = cap;
      }
    }

    const shuffledRivals = rng.shuffle(this.catalog.rivals);
    const rivalObj = shuffledRivals[0];
    const rival: RuleEntry | null = rivalObj
      ? {
          id: rivalObj.id,
          name: rivalObj.name,
          detail: `首领：${rivalObj.boss}`,
        }
      : null;

    const opponents: OpponentPreview[] = ([1, 2, 3] as const).map((plane) => {
      const r = shuffledRivals[plane - 1] ?? rivalObj;
      return {
        plane,
        enemies: (r?.normalEnemies ?? []).slice(0, 3).map((name, idx) => ({
          id: `enemy-${plane}-${idx}`,
          name,
        })),
        bosses: r ? [{ id: r.id, name: r.boss }] : [],
      };
    });

    const affixes: RuleEntry[] = [];
    if (input.affixCount >= 1) {
      const firstEnhancement = rng.pick(this.catalog.enhancementAffixes);
      if (firstEnhancement) {
        affixes.push({
          id: firstEnhancement.id,
          name: firstEnhancement.name,
          detail: firstEnhancement.description,
        });
      }
      const remainingPool = rng.shuffle(this.catalog.regularAffixes);
      for (const extra of remainingPool.slice(0, input.affixCount - 1)) {
        affixes.push({
          id: extra.id,
          name: extra.name,
          detail: extra.description,
        });
      }
    }

    const companionObj = rng.pick(this.catalog.companions);
    const companion: RuleEntry | null = companionObj
      ? { id: companionObj.id, name: companionObj.name }
      : null;

    const envOptions = this.pickEnvironmentOptions(rng, 3);
    const state: InternalMatchState = {
      matchId,
      revision: 0,
      rulesVersion: input.rulesVersion || 's2-v4.7',
      accountId: input.accountId || 'trial-all',
      accountConfigVersion: 'v4.7-trial',
      enemyDifficulty: input.enemyDifficulty,
      seed,
      rng,
      controller: { kind: 'manual' },
      routeIndex: 0,
      enteredTriggerNodes: 0,
      phase: {
        kind: 'investmentEnvironment',
        selectionId: 'env-sel-1',
        options: envOptions,
      },
      rival,
      opponents,
      affixes,
      environment: null,
      companion,
      preparation: null,
      latestSettlement: null,
      poolCounts,
      nextIdCounter: 1,
      activeBattleSeed: null,
      pendingStrategyAfterBattle: false,
      strategySelectedForIndex: {},
      spotlightSelectedForIndex: {},
      actionLog: [
        {
          revision: 0,
          actor: { kind: 'manual' },
          summary: `创建对局 (难度 ${input.enemyDifficulty}, 词缀 ${input.affixCount})`,
        },
      ],
      subscribers: new Set(),
    };

    this.matches.set(matchId, state);
    return ok(this.buildSnapshot(state));
  }

  public async getMatch(input: MatchRef): Promise<MatchResult<MatchSnapshot>> {
    const state = this.matches.get(input.matchId);
    if (!state) {
      return fail({ code: 'matchNotFound', message: `未找到对局 ${input.matchId}` });
    }
    return ok(this.buildSnapshot(state));
  }

  public async setController(
    input: SetControllerInput,
  ): Promise<MatchResult<MatchSnapshot>> {
    const state = this.matches.get(input.matchId);
    if (!state) {
      return fail({ code: 'matchNotFound', message: `未找到对局 ${input.matchId}` });
    }
    if (input.revision !== state.revision) {
      return fail({
        code: 'revisionConflict',
        currentRevision: state.revision,
      });
    }

    const sameKind = state.controller.kind === input.controller.kind;
    const sameModel =
      state.controller.kind !== 'model' ||
      input.controller.kind !== 'model' ||
      state.controller.modelId === input.controller.modelId;

    if (sameKind && sameModel) {
      return ok(this.buildSnapshot(state));
    }

    state.controller = input.controller;
    state.revision += 1;
    state.actionLog.unshift({
      revision: state.revision,
      actor: input.controller,
      summary:
        input.controller.kind === 'manual'
          ? '控制权交还玩家手动操作'
          : `控制权移交模型 (${input.controller.modelId})`,
    });

    this.emit(state, {
      kind: 'controllerChanged',
      matchId: state.matchId,
      revision: state.revision,
      controller: state.controller,
    });
    return ok(this.buildSnapshot(state));
  }

  public async executeAction(
    input: ExecuteActionInput,
    callerController: Controller = { kind: 'manual' },
  ): Promise<MatchResult<MatchSnapshot>> {
    const state = this.matches.get(input.matchId);
    if (!state) {
      return fail({ code: 'matchNotFound', message: `未找到对局 ${input.matchId}` });
    }
    if (input.revision !== state.revision) {
      return fail({
        code: 'revisionConflict',
        currentRevision: state.revision,
      });
    }
    if (state.controller.kind !== callerController.kind) {
      return fail({
        code: 'controllerMismatch',
        controller: state.controller,
      });
    }

    const legal = this.computeLegalActions(state);
    const chosen = legal.find((a) => a.actionId === input.actionId);
    if (!chosen) {
      return fail({
        code: 'actionUnavailable',
        phase: state.phase.kind,
      });
    }

    const summary = this.applyAction(state, chosen.action);
    state.revision += 1;
    state.actionLog.unshift({
      revision: state.revision,
      actor: state.controller,
      summary,
    });
    if (state.actionLog.length > 60) {
      state.actionLog.length = 60;
    }

    this.emit(state, {
      kind: 'actionApplied',
      matchId: state.matchId,
      revision: state.revision,
      actor: state.controller,
      action: chosen.action,
    });

    return ok(this.buildSnapshot(state));
  }

  public async subscribeMatch(
    input: MatchRef,
  ): Promise<MatchResult<AsyncIterable<MatchUpdate>>> {
    const state = this.matches.get(input.matchId);
    if (!state) {
      return fail({ code: 'matchNotFound', message: `未找到对局 ${input.matchId}` });
    }

    const initialSnapshot = this.buildSnapshot(state);
    const queue: MatchUpdate[] = [{ kind: 'snapshot', snapshot: initialSnapshot }];
    let resolver: ((value: IteratorResult<MatchUpdate>) => void) | null = null;
    let closed = false;

    const listener = (update: MatchUpdate): void => {
      if (closed) {
        return;
      }
      if (resolver) {
        const res = resolver;
        resolver = null;
        res({ value: update, done: false });
      } else {
        queue.push(update);
      }
    };

    state.subscribers.add(listener);

    const iterable: AsyncIterable<MatchUpdate> = {
      [Symbol.asyncIterator]: () => ({
        next: (): Promise<IteratorResult<MatchUpdate>> => {
          const queued = queue.shift();
          if (queued) {
            return Promise.resolve({ value: queued, done: false });
          }
          if (closed) {
            return Promise.resolve({ value: undefined, done: true });
          }
          return new Promise<IteratorResult<MatchUpdate>>((resolve) => {
            resolver = resolve;
          });
        },
        return: (): Promise<IteratorResult<MatchUpdate>> => {
          closed = true;
          state.subscribers.delete(listener);
          return Promise.resolve({ value: undefined, done: true });
        },
      }),
    };

    return ok(iterable);
  }

  public async completeBattleAttempt(
    matchId: string,
    completion: BattleCompletion,
  ): Promise<MatchResult<MatchSnapshot>> {
    const state = this.matches.get(matchId);
    if (!state) {
      return fail({ code: 'matchNotFound', message: `未找到对局 ${matchId}` });
    }
    if (
      state.phase.kind !== 'battle' ||
      state.phase.attemptId !== completion.attemptId ||
      !state.preparation
    ) {
      return fail({
        code: 'actionUnavailable',
        phase: state.phase.kind,
      });
    }

    const routeNode = CANONICAL_ROUTE[state.routeIndex];
    if (!routeNode) {
      return fail({ code: 'rulesUnavailable', message: '路线索引越界' });
    }

    const prep = state.preparation;
    const actualProgress =
      completion.progress.kind === 'complete' ? 100 : completion.progress.percent;

    const healthBefore = prep.resources.health;
    const healthCalc = calculateBattleHealthSettlement({
      nodeId: routeNode.nodeId,
      kind: routeNode.kind,
      actualProgress,
      healthBefore,
      maxHealth: prep.resources.maxHealth,
    });

    const won = completion.outcome === 'won' && actualProgress >= 100;
    const hotStreakAfter =
      routeNode.kind === 'reward'
        ? prep.resources.hotStreak
        : won
          ? prep.resources.hotStreak + 1
          : 0;

    const baseGold = 5;
    const interestGold = Math.min(
      prep.resources.interestCap,
      Math.floor(prep.resources.gold / 10),
    );
    const streakGold = calculateStreakGold(hotStreakAfter);
    const experienceGained = 2;

    const grantedCrystals: CrystalStack[] = [];
    for (const drop of completion.crystalDrops) {
      if (drop.count > 0) {
        const added = this.addCrystalToPrep(prep, drop.quality, drop.count);
        grantedCrystals.push(added);
      }
    }

    prep.resources.health = healthCalc.healthAfter;
    prep.resources.gold += baseGold + interestGold + streakGold;
    prep.resources.hotStreak = hotStreakAfter;
    this.grantShopExperience(prep, experienceGained);

    const settlement: NodeSettlement = {
      nodeId: routeNode.nodeId,
      source: 'battle',
      healthBefore,
      healthAfter: healthCalc.healthAfter,
      baseHealthLost: healthCalc.baseHealthLost,
      progressHealthLost: healthCalc.progressHealthLost,
      healthLost: healthCalc.healthLost,
      fixedRecovery: healthCalc.fixedRecovery,
      otherHealthDelta: 0,
      baseGold,
      interestGold,
      streakGold,
      hotStreakAfter,
      experienceGained,
      crystalsGranted: grantedCrystals,
    };

    state.latestSettlement = settlement;
    state.activeBattleSeed = null;

    if (healthCalc.healthAfter <= 0) {
      state.phase = { kind: 'finished', reason: 'healthDepleted' };
    } else if (state.routeIndex >= CANONICAL_ROUTE.length - 1) {
      state.phase = { kind: 'finished', reason: 'cleared' };
    } else {
      this.advanceToNextRouteNode(state);
    }

    state.revision += 1;
    state.actionLog.unshift({
      revision: state.revision,
      actor: state.controller,
      summary: `结算 ${routeNode.nodeId} (${won ? '完胜' : `${Math.floor(actualProgress)}%`}, HP ${healthBefore}→${healthCalc.healthAfter}, +${baseGold + interestGold + streakGold}金)`,
    });

    this.emit(state, {
      kind: 'battleSettled',
      matchId: state.matchId,
      revision: state.revision,
      attemptId: completion.attemptId,
      settlement,
    });

    return ok(this.buildSnapshot(state));
  }

  public getDebugInspect(matchId: string): MatchDebugInspect | null {
    const state = this.matches.get(matchId);
    if (!state) {
      return null;
    }

    const poolRemainingByCost: Record<number, number> = { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 };
    for (const [unitId, count] of Object.entries(state.poolCounts)) {
      const c = this.catalog.charactersById[unitId];
      if (c && c.cost >= 1 && c.cost <= 5) {
        poolRemainingByCost[c.cost] = (poolRemainingByCost[c.cost] ?? 0) + count;
      }
    }

    const estimatedTeamPower = this.estimateTeamPower(state);
    const estimatedNodePower = this.estimateNodePower(state);

    return {
      seed: state.seed,
      activeBattleSeed: state.activeBattleSeed,
      poolRemainingByCost,
      estimatedTeamPower,
      estimatedNodePower,
      actionLog: [...state.actionLog],
    };
  }

  public suggestActions(snapshot: MatchSnapshot): AiRecommendation[] {
    const prep = snapshot.preparation;
    if (!prep || snapshot.phase.kind !== 'preparation') {
      return [];
    }

    const recommendations: AiRecommendation[] = [];

    // 1. Check upgradeable shop offers
    for (const offer of prep.shop.slots) {
      if (!offer) {
        continue;
      }
      const ownedCount = prep.board.pieces.filter((p) => p.unitId === offer.unitId).length;
      const buyAction = snapshot.legalActions.find(
        (a) => a.action.kind === 'buy' && a.action.offerId === offer.offerId,
      );
      if (ownedCount >= 1 && buyAction) {
        const charName = this.catalog.charactersById[offer.unitId]?.name ?? offer.unitId;
        recommendations.push({
          id: buyAction.actionId,
          title: `购买【${charName}】凑升星听牌`,
          tag: ownedCount >= 2 ? '首选 · 可合成升星' : '听牌补强',
          goldCost: offer.price,
          sssDelta: ownedCount >= 2 ? '启发式 · 升星高优' : '启发式 · 听牌',
          rationale: `场上/备战席已持有 ${ownedCount} 个【${charName}】，购入可加快升星节奏。`,
        });
      }
    }

    // 2. Check deployable bench unit if active cap has room
    const activeCap = prep.resources.level;
    if (prep.board.active < activeCap) {
      const benchPiece = prep.board.pieces.find((p) => p.slot.zone === 'bench');
      if (benchPiece) {
        const moveAction = snapshot.legalActions.find(
          (a) =>
            a.action.kind === 'move' &&
            a.action.pieceId === benchPiece.pieceId &&
            a.action.destination.zone !== 'bench',
        );
        if (moveAction) {
          const charName =
            this.catalog.charactersById[benchPiece.unitId]?.name ?? benchPiece.unitId;
          recommendations.push({
            id: moveAction.actionId,
            title: `将【${charName}】部署上场`,
            tag: '人口未满 · 免费提战力',
            sssDelta: '启发式 · 战力',
            rationale: `当前上阵 ${prep.board.active}/${activeCap}，仍有空余人口格位。`,
          });
        }
      }
    }

    // 3. Check item crafting or equip
    if (prep.inventory.length >= 2) {
      const combineAction = snapshot.legalActions.find((a) => a.action.kind === 'combine');
      if (combineAction && combineAction.action.kind === 'combine') {
        const act = combineAction.action;
        const itemA = prep.inventory.find((i) => i.itemId === act.itemId);
        const itemB = prep.inventory.find((i) => i.itemId === act.otherItemId);
        const nameA = itemA ? (this.catalog.itemsById[itemA.definitionId]?.name ?? '') : '';
        const nameB = itemB ? (this.catalog.itemsById[itemB.definitionId]?.name ?? '') : '';
        recommendations.push({
          id: combineAction.actionId,
          title: `合成【${nameA} + ${nameB}】`,
          tag: '装备合成 · 战力跃升',
          sssDelta: '启发式 · 合成',
          rationale: '两件简易装备可直接合成为进阶装备，避免词缀【额外打击】惩罚。',
        });
      }
    }

    // 4. Open crystal if available
    const crystalAction = snapshot.legalActions.find((a) => a.action.kind === 'openCrystal');
    if (crystalAction) {
      recommendations.push({
        id: crystalAction.actionId,
        title: '开启晶矿领取资源',
        tag: '即时资源收益',
        sssDelta: '启发式 · 资源',
        rationale: '备战席未满，可立即开启晶矿补充金币或装备。',
      });
    }

    // 5. Fallback: start battle or buy exp
    if (recommendations.length < 3) {
      const expAction = snapshot.legalActions.find((a) => a.action.kind === 'buyExperience');
      if (expAction && prep.resources.gold >= 14) {
        recommendations.push({
          id: expAction.actionId,
          title: '购买商店经验提升人口',
          tag: '保留利息 · 稳步升本',
          goldCost: 4,
          sssDelta: '启发式 · 经济',
          rationale: '花费 4 金币保持 10+ 金币利息线，同时提升高费卡刷新概率。',
        });
      }
    }

    return recommendations.slice(0, 3);
  }

  private estimateTeamPower(state: InternalMatchState): number {
    const prep = state.preparation;
    if (!prep) {
      return 0;
    }
    let power = 0;
    for (const p of prep.board.pieces) {
      if (p.slot.zone === 'bench') {
        continue;
      }
      const c = this.catalog.charactersById[p.unitId];
      const cost = c?.cost ?? 1;
      const starMult = p.stars === 1 ? 1 : p.stars === 2 ? 2.1 : p.stars === 3 ? 4.4 : 9.0;
      power += (80 + cost * 35) * starMult + p.equipment.length * 45;
    }
    for (const t of prep.traits) {
      if (t.activeThreshold !== null) {
        power += t.activeThreshold * 28;
      }
    }
    return Math.round(power);
  }

  private estimateNodePower(state: InternalMatchState): number {
    const node = CANONICAL_ROUTE[state.routeIndex];
    if (!node) {
      return 0;
    }
    const plane = planeOfNode(node.nodeId);
    const kindMult =
      node.kind === 'boss'
        ? 1.45
        : node.kind === 'encounter'
          ? 1.25
          : node.kind === 'reward'
            ? 0.72
            : 1.0;
    const diffMult = 1 + state.enemyDifficulty / 180;
    const base = 180 + state.routeIndex * 75 + plane * 90;
    return Math.round(base * kindMult * diffMult);
  }

  private pickEnvironmentOptions(
    rng: SeededRng,
    count: number,
  ): RefreshableChoice[] {
    return rng.shuffle(this.catalog.environments).slice(0, count).map((env, idx) => ({
      optionId: `env-opt-${idx + 1}`,
      id: env.id,
      name: env.name,
      detail: env.description,
      refreshAvailable: true,
    }));
  }

  private pickStrategyOptions(
    rng: SeededRng,
    plane: Plane,
    quality: InvestmentQuality,
  ): (RefreshableChoice & { quality: InvestmentQuality })[] {
    const pool = this.catalog.strategiesByQuality[quality].filter((s) =>
      s.planes.includes(plane),
    );
    const source = pool.length >= 3 ? pool : this.catalog.strategiesByQuality[quality];
    return rng.shuffle(source).slice(0, 3).map((st, idx) => ({
      optionId: `st-opt-${idx + 1}`,
      id: st.id,
      name: st.name,
      detail: st.description,
      quality,
      refreshAvailable: true,
    }));
  }

  private pickSupplyOptions(rng: SeededRng, plane: Plane): SupplyOption[] {
    const maxCost = plane === 1 ? 3 : plane === 2 ? 4 : 5;
    const charPool = this.catalog.characters.filter(
      (c) => c.cost >= 1 && c.cost <= maxCost,
    );
    const itemPool =
      plane <= 2 ? this.catalog.basicItemIds : this.catalog.advancedItemIds;

    const chosenChars = rng.shuffle(charPool).slice(0, 5);
    return chosenChars.map((c, idx) => {
      const itemId = rng.pick(itemPool) ?? this.catalog.basicItemIds[0] ?? 'items-001';
      return {
        optionId: `sup-opt-${idx + 1}`,
        unitId: c.id,
        itemDefinitionIds: [itemId],
      };
    });
  }

  private pickSpotlightAuras(rng: SeededRng): { id: string; optionId: string; name: string; detail: string }[] {
    return rng.shuffle(this.catalog.auras).slice(0, 2).map((a, idx) => ({
      optionId: `aura-opt-${idx + 1}`,
      id: a.id,
      name: a.name,
      detail: `${a.description}${a.difficultyIncrease > 0 ? `（敌人难度 +${a.difficultyIncrease}）` : ''}`,
    }));
  }

  private initializeNode11(state: InternalMatchState): void {
    const starterPieces: Piece[] = [];
    const oneCosts = state.rng.shuffle(this.catalog.shopCharactersByCost[1] ?? []);
    const twoCosts = state.rng.shuffle(this.catalog.shopCharactersByCost[2] ?? []);

    const starters = [oneCosts[0], oneCosts[1], twoCosts[0], twoCosts[1]].filter(
      (c): c is NonNullable<typeof c> => c !== undefined,
    );

    let frontIdx = 0;
    let backIdx = 0;
    let benchIdx = 0;

    for (const c of starters) {
      let slot: Slot;
      if (isZoneEligible(c.position, 'front') && frontIdx < 2) {
        slot = { zone: 'front', index: frontIdx };
        frontIdx += 1;
      } else if (isZoneEligible(c.position, 'back') && backIdx < 1) {
        slot = { zone: 'back', index: backIdx };
        backIdx += 1;
      } else {
        slot = { zone: 'bench', index: benchIdx };
        benchIdx += 1;
      }

      const currentPool = state.poolCounts[c.id] ?? 0;
      if (currentPool > 0) {
        state.poolCounts[c.id] = currentPool - 1;
      }

      starterPieces.push({
        pieceId: `piece-${state.nextIdCounter++}`,
        unitId: c.id,
        stars: 1,
        slot,
        equipment: [],
      });
    }

    const prep: Preparation = {
      resources: {
        health: 100,
        maxHealth: 100,
        gold: 3,
        hotStreak: 0,
        interestCap: 5,
        level: 3,
        experience: 0,
        experienceToNextLevel: SHOP_EXP_TO_NEXT[3] ?? null,
      },
      shop: {
        slots: [null, null, null, null, null],
        locked: false,
        freeRefreshes: 0,
      },
      board: {
        front: 0,
        back: 0,
        bench: 0,
        active: 0,
        pieces: starterPieces,
      },
      inventory: [],
      crystals: [
        { crystalId: `cry-${state.nextIdCounter++}`, quality: 'white', count: 1 },
        { crystalId: `cry-${state.nextIdCounter++}`, quality: 'blue', count: 1 },
      ],
      traits: [],
      selectedInvestments: [],
      selectedAuras: [],
      revealedEnemies: this.enemiesForNode(state, FIRST_ROUTE_NODE.nodeId, FIRST_ROUTE_NODE.kind),
      mechanisms: [],
    };

    state.preparation = prep;
    state.routeIndex = 0;
    state.enteredTriggerNodes = 1;
    this.rollShopOffers(state);
    this.recomputeBoardAndTraits(state);
    state.phase = {
      kind: 'preparation',
      spotlightAvailable: false,
      openTraitChoice: null,
    };
  }

  private enemiesForNode(state: InternalMatchState, nodeId: NodeId, kind: NodeKind): RuleEntry[] {
    const plane = planeOfNode(nodeId);
    const opp = state.opponents.find((o) => o.plane === plane);
    if (!opp) {
      return [];
    }
    return kind === 'boss' ? opp.bosses : opp.enemies;
  }

  private rollShopOffers(state: InternalMatchState): void {
    const prep = state.preparation;
    if (!prep) {
      return;
    }
    const odds = getShopOdds(prep.resources.level);
    const slots: (ShopOffer | null)[] = [];

    for (let i = 0; i < 5; i += 1) {
      const roll = state.rng.nextFloat() * 100;
      let cost = 1;
      let acc = odds[1];
      if (roll < acc) {
        cost = 1;
      } else if (roll < (acc += odds[2])) {
        cost = 2;
      } else if (roll < (acc += odds[3])) {
        cost = 3;
      } else if (roll < (acc += odds[4])) {
        cost = 4;
      } else {
        cost = 5;
      }

      const candidates = (this.catalog.shopCharactersByCost[cost] ?? []).filter(
        (c) => (state.poolCounts[c.id] ?? 0) > 0,
      );
      const fallbackCandidates =
        candidates.length > 0
          ? candidates
          : this.catalog.characters.filter(
              (c) => c.cost <= 5 && (state.poolCounts[c.id] ?? 0) > 0,
            );

      const picked = state.rng.pick(fallbackCandidates);
      if (picked) {
        slots.push({
          offerId: `offer-${state.nextIdCounter++}`,
          unitId: picked.id,
          price: picked.cost,
        });
      } else {
        slots.push(null);
      }
    }

    prep.shop.slots = slots;
  }

  private recomputeBoardAndTraits(state: InternalMatchState): void {
    const prep = state.preparation;
    if (!prep) {
      return;
    }
    let front = 0;
    let back = 0;
    let bench = 0;

    const unitSetOnBoard = new Set<string>();
    for (const piece of prep.board.pieces) {
      if (piece.slot.zone === 'front') {
        front += 1;
        unitSetOnBoard.add(piece.unitId);
      } else if (piece.slot.zone === 'back') {
        back += 1;
        unitSetOnBoard.add(piece.unitId);
      } else {
        bench += 1;
      }
    }

    prep.board.front = front;
    prep.board.back = back;
    prep.board.bench = bench;
    prep.board.active = front + back;

    const traitCounts = new Map<string, number>();
    for (const unitId of unitSetOnBoard) {
      const char = this.catalog.charactersById[unitId];
      if (!char) {
        continue;
      }
      for (const traitName of char.traits) {
        traitCounts.set(traitName, (traitCounts.get(traitName) ?? 0) + 1);
      }
    }

    const traits: TraitProgress[] = [];
    for (const [traitName, count] of traitCounts.entries()) {
      const t = this.catalog.traitsByName[traitName];
      if (!t) {
        continue;
      }
      const thresholds =
        t.tiers.length > 0 ? t.tiers.map((tier) => tier.level) : [2, 4, 6];
      let activeThreshold: number | null = null;
      for (const th of thresholds) {
        if (count >= th) {
          activeThreshold = th;
        }
      }
      traits.push({
        traitId: t.id,
        count,
        activeThreshold,
        thresholds,
      });
    }

    traits.sort((a, b) => {
      const aActive = a.activeThreshold !== null ? 1 : 0;
      const bActive = b.activeThreshold !== null ? 1 : 0;
      if (aActive !== bActive) {
        return bActive - aActive;
      }
      return b.count - a.count;
    });

    prep.traits = traits;
  }

  private tryAutoMerge(state: InternalMatchState, unitId: string): boolean {
    const prep = state.preparation;
    if (!prep) {
      return false;
    }

    let mergedAny = false;
    let keepChecking = true;
    while (keepChecking) {
      keepChecking = false;
      for (const starLevel of [1, 2]) {
        const matches = prep.board.pieces.filter(
          (p) => p.unitId === unitId && p.stars === starLevel,
        );
        if (matches.length >= 3) {
          const triplet = matches.slice(0, 3);
          const anchor =
            triplet.find((p) => p.slot.zone !== 'bench') ?? triplet[0];
          if (!anchor) {
            break;
          }
          const combinedEquip: ItemStack[] = [];
          const returnedEquip: ItemStack[] = [];

          for (const member of triplet) {
            for (const eq of member.equipment) {
              if (combinedEquip.length < 3) {
                combinedEquip.push(eq);
              } else {
                returnedEquip.push(eq);
              }
            }
          }

          const dropIds = new Set(
            triplet.filter((m) => m.pieceId !== anchor.pieceId).map((m) => m.pieceId),
          );
          prep.board.pieces = prep.board.pieces.filter((p) => !dropIds.has(p.pieceId));
          anchor.stars = starLevel + 1;
          anchor.equipment = combinedEquip;
          for (const ret of returnedEquip) {
            this.addItemToInventory(prep, ret.definitionId, ret.count);
          }

          mergedAny = true;
          keepChecking = true;
          break;
        }
      }
    }

    if (mergedAny) {
      this.recomputeBoardAndTraits(state);
    }
    return mergedAny;
  }

  private addItemToInventory(
    prep: Preparation,
    definitionId: string,
    count = 1,
  ): void {
    const existing = prep.inventory.find((i) => i.definitionId === definitionId);
    if (existing) {
      existing.count += count;
    } else {
      prep.inventory.push({
        itemId: `inv-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
        definitionId,
        count,
      });
    }
  }

  private consumeInventoryItem(prep: Preparation, itemId: string): ItemStack | null {
    const idx = prep.inventory.findIndex((i) => i.itemId === itemId);
    if (idx === -1) {
      return null;
    }
    const stack = prep.inventory[idx];
    if (!stack) {
      return null;
    }
    if (stack.count > 1) {
      stack.count -= 1;
    } else {
      prep.inventory.splice(idx, 1);
    }
    return {
      itemId: `eq-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
      definitionId: stack.definitionId,
      count: 1,
    };
  }

  private addCrystalToPrep(
    prep: Preparation,
    quality: CrystalQuality,
    count: number,
  ): CrystalStack {
    const existing = prep.crystals.find((c) => c.quality === quality);
    if (existing) {
      existing.count += count;
      return { ...existing, count };
    }
    const created: CrystalStack = {
      crystalId: `cry-${quality}-${Date.now().toString(36)}`,
      quality,
      count,
    };
    prep.crystals.push(created);
    return { ...created };
  }

  private grantShopExperience(prep: Preparation, amount: number): void {
    if (prep.resources.level >= 10) {
      return;
    }
    prep.resources.experience += amount;
    while (prep.resources.level < 10) {
      const needed = SHOP_EXP_TO_NEXT[prep.resources.level];
      if (needed === null || needed === undefined || prep.resources.experience < needed) {
        break;
      }
      prep.resources.experience -= needed;
      prep.resources.level += 1;
    }
    prep.resources.experienceToNextLevel = SHOP_EXP_TO_NEXT[prep.resources.level] ?? null;
  }

  private advanceToNextRouteNode(state: InternalMatchState): void {
    const prep = state.preparation;
    if (!prep) {
      return;
    }
    state.routeIndex += 1;
    const nextNode = CANONICAL_ROUTE[state.routeIndex];
    if (!nextNode) {
      state.phase = { kind: 'finished', reason: 'cleared' };
      return;
    }

    prep.revealedEnemies = this.enemiesForNode(state, nextNode.nodeId, nextNode.kind);

    if (!prep.shop.locked) {
      this.rollShopOffers(state);
    } else {
      prep.shop.locked = false;
    }

    if (INVESTMENT_STRATEGY_NODES.has(nextNode.nodeId)) {
      const qualityRoll = state.rng.nextFloat();
      const quality: InvestmentQuality =
        qualityRoll < 0.25 ? 'prismatic' : qualityRoll < 0.65 ? 'gold' : 'silver';
      const plane = planeOfNode(nextNode.nodeId);
      state.phase = {
        kind: 'investmentStrategy',
        selectionId: `st-sel-${nextNode.nodeId}`,
        view: 'selection',
        options: this.pickStrategyOptions(state.rng, plane, quality),
      };
      return;
    }

    if (nextNode.kind === 'supply') {
      const plane = planeOfNode(nextNode.nodeId);
      state.phase = {
        kind: 'supply',
        selectionId: `sup-sel-${nextNode.nodeId}`,
        view: 'selection',
        options: this.pickSupplyOptions(state.rng, plane),
        refreshAvailable: true,
      };
      return;
    }

    state.enteredTriggerNodes += 1;
    state.phase = {
      kind: 'preparation',
      spotlightAvailable:
        SPOTLIGHT_BOSS_NODES.has(nextNode.nodeId) &&
        !state.spotlightSelectedForIndex[state.routeIndex],
      openTraitChoice: null,
    };
  }

  private applyAction(state: InternalMatchState, action: MatchAction): string {
    const prep = state.preparation;

    switch (action.kind) {
      case 'refreshChoice': {
        if (state.phase.kind === 'investmentEnvironment') {
          const idx = state.phase.options.findIndex((o) => o.optionId === action.optionId);
          const usedIds = new Set(state.phase.options.map((o) => o.id));
          const replacement = state.rng.pick(
            this.catalog.environments.filter((e) => !usedIds.has(e.id)),
          );
          if (idx !== -1 && replacement) {
            state.phase.options[idx] = {
              optionId: action.optionId,
              id: replacement.id,
              name: replacement.name,
              detail: replacement.description,
              refreshAvailable: false,
            };
            return `刷新投资环境候选为【${replacement.name}】`;
          }
        }
        if (state.phase.kind === 'investmentStrategy') {
          const idx = state.phase.options.findIndex((o) => o.optionId === action.optionId);
          const target = state.phase.options[idx];
          if (idx !== -1 && target) {
            const usedIds = new Set(state.phase.options.map((o) => o.id));
            const plane = planeOfNode(
              CANONICAL_ROUTE[state.routeIndex]?.nodeId ?? '1-3',
            );
            const pool = this.catalog.strategiesByQuality[target.quality].filter(
              (s) => !usedIds.has(s.id) && s.planes.includes(plane),
            );
            const replacement = state.rng.pick(
              pool.length > 0
                ? pool
                : this.catalog.strategiesByQuality[target.quality].filter(
                    (s) => !usedIds.has(s.id),
                  ),
            );
            if (replacement) {
              state.phase.options[idx] = {
                optionId: action.optionId,
                id: replacement.id,
                name: replacement.name,
                detail: replacement.description,
                quality: target.quality,
                refreshAvailable: false,
              };
              return `刷新投资策略候选为【${replacement.name}】`;
            }
          }
        }
        if (state.phase.kind === 'spotlight' && prep) {
          prep.resources.gold -= state.phase.refreshPrice;
          state.phase.refreshCount += 1;
          state.phase.refreshPrice =
            state.phase.refreshCount >= state.phase.discountedRefreshLimit ? 2 : 1;
          const idx = state.phase.options.findIndex((o) => o.optionId === action.optionId);
          const usedIds = new Set(state.phase.options.map((o) => o.id));
          const replacement = state.rng.pick(
            this.catalog.auras.filter((a) => !usedIds.has(a.id)),
          );
          if (idx !== -1 && replacement) {
            state.phase.options[idx] = {
              optionId: action.optionId,
              id: replacement.id,
              name: replacement.name,
              detail: replacement.description,
            };
            return `刷新主角光环候选为【${replacement.name}】`;
          }
        }
        return '刷新候选';
      }

      case 'chooseInvestmentEnvironment': {
        if (state.phase.kind === 'investmentEnvironment') {
          const opt = state.phase.options.find((o) => o.optionId === action.optionId);
          if (opt) {
            state.environment = { id: opt.id, name: opt.name, detail: opt.detail };
            this.initializeNode11(state);
            return `选定投资环境【${opt.name}】，进入 1-1`;
          }
        }
        return '选定投资环境';
      }

      case 'returnToPreparation': {
        if (
          state.phase.kind === 'investmentStrategy' ||
          state.phase.kind === 'supply' ||
          state.phase.kind === 'spotlight'
        ) {
          state.phase.view = 'preparation';
          return '暂时返回备战视图';
        }
        return '返回备战视图';
      }

      case 'returnToSelection': {
        if (
          state.phase.kind === 'investmentStrategy' ||
          state.phase.kind === 'supply' ||
          state.phase.kind === 'spotlight'
        ) {
          state.phase.view = 'selection';
          return '回到选择界面';
        }
        return '回到选择界面';
      }

      case 'chooseInvestmentStrategy': {
        if (state.phase.kind === 'investmentStrategy' && prep) {
          const opt = state.phase.options.find((o) => o.optionId === action.optionId);
          if (opt) {
            prep.selectedInvestments.push({
              id: opt.id,
              name: opt.name,
              detail: opt.detail,
            });
            // Apply representative immediate economy bonuses if mentioned
            const goldMatch = opt.detail?.match(/(?:立刻)?获得(\d+)(?:枚|个)?金币/);
            if (goldMatch?.[1]) {
              prep.resources.gold += Number(goldMatch[1]);
            }
            if (opt.detail?.includes('利息上限提升至9')) {
              prep.resources.interestCap = 9;
            } else if (opt.detail?.includes('最大利息提升至10')) {
              prep.resources.interestCap = 10;
            }
            state.strategySelectedForIndex[state.routeIndex] = true;
            state.enteredTriggerNodes += 1;
            state.phase = {
              kind: 'preparation',
              spotlightAvailable: false,
              openTraitChoice: null,
            };
            return `选定投资策略【${opt.name}】，正式进入 ${CANONICAL_ROUTE[state.routeIndex]?.nodeId ?? ''}`;
          }
        }
        return '选定投资策略';
      }

      case 'refreshSupply': {
        if (state.phase.kind === 'supply') {
          const plane = planeOfNode(CANONICAL_ROUTE[state.routeIndex]?.nodeId ?? '1-5');
          state.phase.options = this.pickSupplyOptions(state.rng, plane);
          state.phase.refreshAvailable = false;
          return '刷新补给候选组合';
        }
        return '刷新补给';
      }

      case 'takeSupply': {
        if (state.phase.kind === 'supply' && prep) {
          const opt = state.phase.options.find((o) => o.optionId === action.optionId);
          if (opt) {
            const benchSlot = this.findFirstEmptyBenchIndex(prep);
            if (benchSlot !== null) {
              const pool = state.poolCounts[opt.unitId] ?? 0;
              if (pool > 0) {
                state.poolCounts[opt.unitId] = pool - 1;
              }
              prep.board.pieces.push({
                pieceId: `piece-${state.nextIdCounter++}`,
                unitId: opt.unitId,
                stars: 1,
                slot: { zone: 'bench', index: benchSlot },
                equipment: [],
              });
              this.tryAutoMerge(state, opt.unitId);
            }
            for (const itemDef of opt.itemDefinitionIds) {
              this.addItemToInventory(prep, itemDef, 1);
            }
            this.recomputeBoardAndTraits(state);
            const charName = this.catalog.charactersById[opt.unitId]?.name ?? opt.unitId;
            this.advanceToNextRouteNode(state);
            return `领取补给【${charName}】并自动推进`;
          }
        }
        return '领取补给';
      }

      case 'openSpotlight': {
        if (state.phase.kind === 'preparation' && prep) {
          const eligiblePieceIds = prep.board.pieces
            .filter((p) => p.slot.zone !== 'bench')
            .map((p) => p.pieceId);
          state.phase = {
            kind: 'spotlight',
            selectionId: `spotlight-${state.routeIndex}`,
            view: 'selection',
            options: this.pickSpotlightAuras(state.rng),
            eligiblePieceIds,
            refreshCount: 0,
            discountedRefreshLimit: 6,
            refreshPrice: 1,
          };
          return '打开主角光环选择';
        }
        return '打开主角光环';
      }

      case 'chooseSpotlight': {
        if (state.phase.kind === 'spotlight' && prep) {
          const opt = state.phase.options.find((o) => o.optionId === action.optionId);
          const auraObj = opt ? this.catalog.aurasById[opt.id] : undefined;
          if (opt && auraObj) {
            for (const existing of prep.selectedAuras) {
              if (existing.pieceId === action.pieceId) {
                existing.effectActive = false;
              }
            }
            prep.selectedAuras.push({
              id: auraObj.id,
              name: auraObj.name,
              detail: auraObj.description,
              pieceId: action.pieceId,
              effectActive: true,
              difficultyIncrease: auraObj.difficultyIncrease,
            });
            state.enemyDifficulty = Math.min(
              299,
              state.enemyDifficulty + auraObj.difficultyIncrease,
            );
            state.spotlightSelectedForIndex[state.routeIndex] = true;
            state.phase = {
              kind: 'preparation',
              spotlightAvailable: false,
              openTraitChoice: null,
            };
            return `为角色赋予主角光环【${auraObj.name}】`;
          }
        }
        return '选择主角光环';
      }

      case 'buy': {
        if (!prep) {
          return '购买失败';
        }
        const slotIdx = prep.shop.slots.findIndex((s) => s?.offerId === action.offerId);
        const offer = prep.shop.slots[slotIdx];
        if (slotIdx === -1 || !offer) {
          return '报价已失效';
        }
        prep.resources.gold -= offer.price;
        prep.shop.slots[slotIdx] = null;
        const pool = state.poolCounts[offer.unitId] ?? 0;
        if (pool > 0) {
          state.poolCounts[offer.unitId] = pool - 1;
        }
        const benchSlot = this.findFirstEmptyBenchIndex(prep) ?? 8;
        prep.board.pieces.push({
          pieceId: `piece-${state.nextIdCounter++}`,
          unitId: offer.unitId,
          stars: 1,
          slot: { zone: 'bench', index: benchSlot },
          equipment: [],
        });
        const merged = this.tryAutoMerge(state, offer.unitId);
        this.recomputeBoardAndTraits(state);
        const charName = this.catalog.charactersById[offer.unitId]?.name ?? offer.unitId;
        return `购买【${charName}】(-${offer.price}金)${merged ? ' 并触发升星' : ''}`;
      }

      case 'sell': {
        if (!prep) {
          return '出售失败';
        }
        const idx = prep.board.pieces.findIndex((p) => p.pieceId === action.pieceId);
        const piece = prep.board.pieces[idx];
        if (idx === -1 || !piece) {
          return '角色不存在';
        }
        const char = this.catalog.charactersById[piece.unitId];
        const gain = sellGoldValue(char?.cost ?? 1, piece.stars);
        prep.resources.gold += gain;
        const copies = pieceCopiesCount(piece.stars);
        state.poolCounts[piece.unitId] = (state.poolCounts[piece.unitId] ?? 0) + copies;
        for (const eq of piece.equipment) {
          this.addItemToInventory(prep, eq.definitionId, eq.count);
        }
        prep.board.pieces.splice(idx, 1);
        this.recomputeBoardAndTraits(state);
        return `出售【${char?.name ?? piece.unitId}】(+${gain}金)`;
      }

      case 'move': {
        if (!prep) {
          return '移动失败';
        }
        const piece = prep.board.pieces.find((p) => p.pieceId === action.pieceId);
        if (!piece) {
          return '角色不存在';
        }
        const occupant = prep.board.pieces.find(
          (p) =>
            p.slot.zone === action.destination.zone &&
            p.slot.index === action.destination.index,
        );
        const prevSlot = piece.slot;
        piece.slot = { ...action.destination };
        if (occupant) {
          occupant.slot = prevSlot;
        }
        this.recomputeBoardAndTraits(state);
        const charName = this.catalog.charactersById[piece.unitId]?.name ?? piece.unitId;
        return occupant
          ? `交换【${charName}】至 ${action.destination.zone}#${action.destination.index + 1}`
          : `移动【${charName}】至 ${action.destination.zone}#${action.destination.index + 1}`;
      }

      case 'refreshShop': {
        if (!prep) {
          return '刷新失败';
        }
        if (prep.shop.freeRefreshes > 0) {
          prep.shop.freeRefreshes -= 1;
        } else {
          prep.resources.gold -= 2;
        }
        this.rollShopOffers(state);
        return '刷新商店';
      }

      case 'buyExperience': {
        if (!prep) {
          return '购买经验失败';
        }
        prep.resources.gold -= 4;
        this.grantShopExperience(prep, 4);
        return `购买经验 (-4金, Lv.${prep.resources.level})`;
      }

      case 'lockShop': {
        if (!prep) {
          return '锁定失败';
        }
        prep.shop.locked = action.locked;
        return action.locked ? '锁定商店' : '解锁商店';
      }

      case 'openCrystal': {
        if (!prep) {
          return '开启失败';
        }
        const crystalIdx = prep.crystals.findIndex(
          (c) => c.crystalId === action.crystalId,
        );
        const crystal = prep.crystals[crystalIdx];
        if (crystalIdx === -1 || !crystal) {
          return '晶矿不存在';
        }
        if (crystal.count > 1) {
          crystal.count -= 1;
        } else {
          prep.crystals.splice(crystalIdx, 1);
        }

        const goldGain =
          crystal.quality === 'white'
            ? 2
            : crystal.quality === 'blue'
              ? 4
              : crystal.quality === 'gold'
                ? 7
                : 12;
        prep.resources.gold += goldGain;

        const itemPool =
          crystal.quality === 'white' || crystal.quality === 'blue'
            ? this.catalog.basicItemIds
            : this.catalog.advancedItemIds;
        const itemDef = state.rng.pick(itemPool);
        let itemLabel = '';
        if (itemDef) {
          this.addItemToInventory(prep, itemDef, 1);
          itemLabel = ` + 【${this.catalog.itemsById[itemDef]?.name ?? itemDef}】`;
        }

        return `开启${CRYSTAL_LABELS[crystal.quality]}：+${goldGain}金${itemLabel}`;
      }

      case 'combine': {
        if (!prep) {
          return '合成失败';
        }
        const stackA = prep.inventory.find((i) => i.itemId === action.itemId);
        const stackB = prep.inventory.find((i) => i.itemId === action.otherItemId);
        if (!stackA || !stackB) {
          return '材料不足';
        }
        const pair = [stackA.definitionId, stackB.definitionId].sort();
        const resultId = this.catalog.recipeLookup[`${pair[0]}+${pair[1]}`];
        if (!resultId) {
          return '无效合成配方';
        }
        this.consumeInventoryItem(prep, action.itemId);
        this.consumeInventoryItem(prep, action.otherItemId);
        this.addItemToInventory(prep, resultId, 1);
        const resultName = this.catalog.itemsById[resultId]?.name ?? resultId;
        return `合成进阶装备【${resultName}】`;
      }

      case 'equip': {
        if (!prep) {
          return '装备失败';
        }
        const piece = prep.board.pieces.find((p) => p.pieceId === action.pieceId);
        const invStack = prep.inventory.find((i) => i.itemId === action.itemId);
        if (!piece || !invStack) {
          return '目标无效';
        }
        const consumed = this.consumeInventoryItem(prep, action.itemId);
        if (!consumed) {
          return '物品不足';
        }

        // Check on-unit basic item combination
        const itemInfo = this.catalog.itemsById[consumed.definitionId];
        if (itemInfo?.type === '简易装备') {
          const matchIdx = piece.equipment.findIndex((eq) => {
            const pair = [eq.definitionId, consumed.definitionId].sort();
            return this.catalog.recipeLookup[`${pair[0]}+${pair[1]}`] !== undefined;
          });
          const baseEq = piece.equipment[matchIdx];
          if (matchIdx !== -1 && baseEq) {
            const pair = [baseEq.definitionId, consumed.definitionId].sort();
            const craftedId = this.catalog.recipeLookup[`${pair[0]}+${pair[1]}`];
            if (craftedId) {
              piece.equipment[matchIdx] = {
                itemId: `eq-${state.nextIdCounter++}`,
                definitionId: craftedId,
                count: 1,
              };
              const craftedName = this.catalog.itemsById[craftedId]?.name ?? craftedId;
              return `身上合成进阶装备【${craftedName}】`;
            }
          }
        }

        piece.equipment.push(consumed);
        const charName = this.catalog.charactersById[piece.unitId]?.name ?? piece.unitId;
        return `为【${charName}】装备【${itemInfo?.name ?? consumed.definitionId}】`;
      }

      case 'startBattle': {
        const attemptId = `attempt-${state.nextIdCounter++}`;
        state.activeBattleSeed = `${state.seed}:node-${state.routeIndex}:enter-${state.nextIdCounter}`;
        state.phase = { kind: 'battle', attemptId };
        return `进入节点 ${CANONICAL_ROUTE[state.routeIndex]?.nodeId ?? ''} 战斗`;
      }

      case 'restartBattle': {
        const attemptId = `attempt-${state.nextIdCounter++}`;
        // Preserves activeBattleSeed per contract.md
        state.phase = { kind: 'battle', attemptId };
        return '战斗中直接重开（沿用本次随机序列）';
      }

      case 'leaveBattle': {
        state.phase = { kind: 'battleAway' };
        return '主动暂离战斗（可调整站位）';
      }

      case 'reenterBattle': {
        const attemptId = `attempt-${state.nextIdCounter++}`;
        state.activeBattleSeed = `${state.seed}:reenter-${state.nextIdCounter}`;
        state.phase = { kind: 'battle', attemptId };
        return '重新进入战斗（使用新随机序列）';
      }

      default:
        return '执行操作';
    }
  }

  private findFirstEmptyBenchIndex(prep: Preparation): number | null {
    const used = new Set(
      prep.board.pieces.filter((p) => p.slot.zone === 'bench').map((p) => p.slot.index),
    );
    for (let i = 0; i < 9; i += 1) {
      if (!used.has(i)) {
        return i;
      }
    }
    return null;
  }

  private canBuyOffer(prep: Preparation, offer: ShopOffer): boolean {
    if (prep.resources.gold < offer.price) {
      return false;
    }
    if (prep.board.bench < 9) {
      return true;
    }
    // Full bench is allowed if buying triggers a 3-of-a-kind merge
    const sameOneStar = prep.board.pieces.filter(
      (p) => p.unitId === offer.unitId && p.stars === 1,
    ).length;
    return sameOneStar >= 2;
  }

  private canMovePiece(
    prep: Preparation,
    piece: Piece,
    dest: Slot,
  ): boolean {
    if (piece.slot.zone === dest.zone && piece.slot.index === dest.index) {
      return false;
    }
    const char = this.catalog.charactersById[piece.unitId];
    if (!char || !isZoneEligible(char.position, dest.zone)) {
      return false;
    }

    const occupant = prep.board.pieces.find(
      (p) => p.slot.zone === dest.zone && p.slot.index === dest.index,
    );
    if (occupant) {
      const occChar = this.catalog.charactersById[occupant.unitId];
      if (!occChar || !isZoneEligible(occChar.position, piece.slot.zone)) {
        return false;
      }
      return true;
    }

    // Moving from bench to empty front/back requires active cap
    if (piece.slot.zone === 'bench' && dest.zone !== 'bench') {
      return prep.board.active < prep.resources.level;
    }
    return true;
  }

  private computeLegalActions(state: InternalMatchState): LegalAction[] {
    const actions: LegalAction[] = [];
    let counter = 1;
    const push = (
      action: MatchAction,
      goldCost?: number,
    ): void => {
      actions.push({
        actionId: `act-${counter++}`,
        action,
        goldCost,
      });
    };

    const phase = state.phase;
    const prep = state.preparation;

    if (phase.kind === 'investmentEnvironment') {
      for (const opt of phase.options) {
        if (opt.refreshAvailable) {
          push({
            kind: 'refreshChoice',
            choice: 'investmentEnvironment',
            selectionId: phase.selectionId,
            optionId: opt.optionId,
          });
        }
        push({
          kind: 'chooseInvestmentEnvironment',
          selectionId: phase.selectionId,
          optionId: opt.optionId,
        });
      }
      return actions;
    }

    if (phase.kind === 'investmentStrategy') {
      if (phase.view === 'selection') {
        for (const opt of phase.options) {
          if (opt.refreshAvailable) {
            push({
              kind: 'refreshChoice',
              choice: 'investmentStrategy',
              selectionId: phase.selectionId,
              optionId: opt.optionId,
            });
          }
          push({
            kind: 'chooseInvestmentStrategy',
            selectionId: phase.selectionId,
            optionId: opt.optionId,
          });
        }
        push({ kind: 'returnToPreparation', selectionId: phase.selectionId });
      } else {
        push({ kind: 'returnToSelection', selectionId: phase.selectionId });
        if (prep && prep.board.bench < 9) {
          for (const cry of prep.crystals) {
            push({ kind: 'openCrystal', crystalId: cry.crystalId });
          }
        }
      }
      return actions;
    }

    if (phase.kind === 'supply') {
      if (phase.view === 'selection') {
        if (phase.refreshAvailable) {
          push({ kind: 'refreshSupply', selectionId: phase.selectionId });
        }
        for (const opt of phase.options) {
          push({
            kind: 'takeSupply',
            selectionId: phase.selectionId,
            optionId: opt.optionId,
          });
        }
        push({ kind: 'returnToPreparation', selectionId: phase.selectionId });
      } else {
        push({ kind: 'returnToSelection', selectionId: phase.selectionId });
        if (prep && prep.board.bench < 9) {
          for (const cry of prep.crystals) {
            push({ kind: 'openCrystal', crystalId: cry.crystalId });
          }
        }
      }
      return actions;
    }

    if (phase.kind === 'spotlight') {
      if (phase.view === 'selection') {
        if (prep && prep.resources.gold >= phase.refreshPrice) {
          for (const opt of phase.options) {
            push(
              {
                kind: 'refreshChoice',
                choice: 'spotlight',
                selectionId: phase.selectionId,
                optionId: opt.optionId,
              },
              phase.refreshPrice,
            );
          }
        }
        for (const opt of phase.options) {
          for (const pieceId of phase.eligiblePieceIds) {
            push({
              kind: 'chooseSpotlight',
              selectionId: phase.selectionId,
              optionId: opt.optionId,
              pieceId,
            });
          }
        }
        push({ kind: 'returnToPreparation', selectionId: phase.selectionId });
      } else {
        push({ kind: 'returnToSelection', selectionId: phase.selectionId });
        if (prep) {
          this.appendMoveActions(prep, push);
        }
      }
      return actions;
    }

    if (phase.kind === 'battle') {
      push({ kind: 'restartBattle' });
      push({ kind: 'leaveBattle' });
      return actions;
    }

    if (phase.kind === 'battleAway') {
      push({ kind: 'reenterBattle' });
      if (prep) {
        this.appendMoveActions(prep, push);
      }
      return actions;
    }

    if (phase.kind === 'preparation' && prep) {
      // Buy offers
      for (const offer of prep.shop.slots) {
        if (offer && this.canBuyOffer(prep, offer)) {
          push({ kind: 'buy', offerId: offer.offerId }, offer.price);
        }
      }
      // Sell pieces
      for (const piece of prep.board.pieces) {
        push({ kind: 'sell', pieceId: piece.pieceId });
      }
      // Move pieces
      this.appendMoveActions(prep, push);
      // Shop refresh / exp / lock
      if (prep.shop.freeRefreshes > 0 || prep.resources.gold >= 2) {
        push(
          { kind: 'refreshShop' },
          prep.shop.freeRefreshes > 0 ? 0 : 2,
        );
      }
      if (prep.resources.level < 10 && prep.resources.gold >= 4) {
        push({ kind: 'buyExperience' }, 4);
      }
      push({ kind: 'lockShop', locked: !prep.shop.locked });

      // Crystals (blocked if bench == 9)
      if (prep.board.bench < 9) {
        for (const cry of prep.crystals) {
          push({ kind: 'openCrystal', crystalId: cry.crystalId });
        }
      }

      // Inventory combine
      for (let i = 0; i < prep.inventory.length; i += 1) {
        const a = prep.inventory[i];
        if (!a) {
          continue;
        }
        for (let j = i; j < prep.inventory.length; j += 1) {
          const b = prep.inventory[j];
          if (!b) {
            continue;
          }
          if (i === j && a.count < 2) {
            continue;
          }
          const pair = [a.definitionId, b.definitionId].sort();
          if (this.catalog.recipeLookup[`${pair[0]}+${pair[1]}`]) {
            push({ kind: 'combine', itemId: a.itemId, otherItemId: b.itemId });
          }
        }
      }

      // Equip items onto pieces
      for (const inv of prep.inventory) {
        const info = this.catalog.itemsById[inv.definitionId];
        for (const piece of prep.board.pieces) {
          const canOnUnitCombine =
            info?.type === '简易装备' &&
            piece.equipment.some((eq) => {
              const pair = [eq.definitionId, inv.definitionId].sort();
              return this.catalog.recipeLookup[`${pair[0]}+${pair[1]}`] !== undefined;
            });
          if (piece.equipment.length < 3 || canOnUnitCombine) {
            push({ kind: 'equip', itemId: inv.itemId, pieceId: piece.pieceId });
          }
        }
      }

      if (phase.spotlightAvailable && prep.board.active > 0) {
        push({ kind: 'openSpotlight' });
      }
      push({ kind: 'startBattle' });
    }

    return actions;
  }

  private appendMoveActions(
    prep: Preparation,
    push: (action: MatchAction) => void,
  ): void {
    const allSlots: Slot[] = [
      ...Array.from({ length: 4 }, (_, index) => ({ zone: 'front' as const, index })),
      ...Array.from({ length: 4 }, (_, index) => ({ zone: 'back' as const, index })),
      ...Array.from({ length: 9 }, (_, index) => ({ zone: 'bench' as const, index })),
    ];

    for (const piece of prep.board.pieces) {
      for (const dest of allSlots) {
        if (this.canMovePiece(prep, piece, dest)) {
          push({ kind: 'move', pieceId: piece.pieceId, destination: dest });
        }
      }
    }
  }

  private buildSnapshot(state: InternalMatchState): MatchSnapshot {
    const currentNode = CANONICAL_ROUTE[state.routeIndex];
    return cloneSnapshotData({
      matchId: state.matchId,
      revision: state.revision,
      rulesVersion: state.rulesVersion,
      accountId: state.accountId,
      accountConfigVersion: state.accountConfigVersion,
      controller: state.controller,
      route: [...CANONICAL_ROUTE],
      position: currentNode
        ? {
            nodeId: currentNode.nodeId,
            kind: currentNode.kind,
            index: state.routeIndex,
            enteredTriggerNodes: state.enteredTriggerNodes,
          }
        : null,
      phase: state.phase,
      rival: state.rival,
      opponents: state.opponents,
      affixes: state.affixes,
      environment: state.environment,
      companion: state.companion,
      preparation: state.preparation,
      legalActions: this.computeLegalActions(state),
      latestSettlement: state.latestSettlement,
    });
  }

  private emit(state: InternalMatchState, update: MatchUpdate): void {
    for (const sub of state.subscribers) {
      sub(cloneSnapshotData(update));
    }
  }
}
