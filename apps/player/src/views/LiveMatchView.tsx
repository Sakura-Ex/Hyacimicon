import { useEffect, useMemo, useRef, useState } from 'react';
import { COIN_ICON_URL, resourceIconUrl } from '../assets/paths';
import { CharacterCard } from '../components/domain/CharacterCard';
import { CrystalBadge } from '../components/domain/CrystalBadge';
import { InvestmentCard } from '../components/domain/InvestmentCard';
import { ItemFrameIcon } from '../components/domain/ItemFrameIcon';
import { RouteTimeline } from '../components/domain/RouteTimeline';
import { TraitBadge } from '../components/domain/TraitBadge';
import { CostBadge, HsrTag, PositionBadge } from '../components/primitives/HsrBadge';
import { HsrButton } from '../components/primitives/HsrButton';
import { HsrPanel } from '../components/primitives/HsrPanel';
import { HsrRichText } from '../components/primitives/HsrRichText';
import {
  type EncyclopediaCatalog,
  loadEncyclopediaCatalog,
} from '../data/encyclopediaLoader';
import type {
  CrystalQuality,
  LegalAction,
  MatchDebugInspect,
  MatchSnapshot,
  Piece,
  RouteNode,
  Slot,
} from '../match/contract';
import { LocalSimulatedMatchService } from '../match/localMatchService';
import {
  calculateBattleHealthSettlement,
  getShopOdds,
  sellGoldValue,
} from '../match/rules';
import type {
  RouteNodeItem,
  ShowcaseCharacter,
  ShowcaseItem,
  ShowcaseTrait,
  TraitTier,
} from '../types/hsr';

function toTraitTier(
  quality: string | undefined,
  active: boolean,
): TraitTier {
  if (!active) {
    return 'inactive';
  }
  switch (quality) {
    case 'Silver':
      return 'silver';
    case 'Gold':
      return 'gold';
    case 'Multicolor':
      return 'prismatic';
    default:
      return 'bronze';
  }
}

function toShowcaseCharacter(
  catalog: EncyclopediaCatalog,
  unitId: string,
  stars: number,
): ShowcaseCharacter {
  const c = catalog.charactersById[unitId];
  return {
    id: unitId,
    name: c?.name ?? unitId,
    cost: c?.cost ?? 1,
    position: c?.position ?? '前台',
    roles: c?.roles ?? [],
    traits: c?.traits ?? [],
    portraitPath: c?.portraitPath ?? '',
    stars: stars >= 4 ? 4 : stars === 3 ? 3 : stars === 2 ? 2 : 1,
    hp: 2200 + (c?.cost ?? 1) * 320,
    speed: 110 + (c?.cost ?? 1) * 8,
  };
}

function toShowcaseItem(
  catalog: EncyclopediaCatalog,
  definitionId: string,
  count?: number,
): ShowcaseItem {
  const item = catalog.itemsById[definitionId];
  return {
    id: definitionId,
    name: item?.name ?? definitionId,
    type: item?.type ?? '装备',
    frame: item?.frame ?? 'normal',
    imagePath: item?.imagePath ?? '',
    description: item?.description ?? '',
    count,
  };
}

const CRYSTAL_LABELS: Record<CrystalQuality, string> = {
  white: '白色晶矿',
  blue: '蓝色晶矿',
  gold: '黄金晶矿',
  prismatic: '棱彩晶矿',
};

export function LiveMatchView() {
  const serviceRef = useRef<LocalSimulatedMatchService | null>(null);
  const [catalog, setCatalog] = useState<EncyclopediaCatalog | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<MatchSnapshot | null>(null);
  const [inspect, setInspect] = useState<MatchDebugInspect | null>(null);

  const [selectedPieceId, setSelectedPieceId] = useState<string | null>(null);
  const [selectedInvItemId, setSelectedInvItemId] = useState<string | null>(null);
  const [selectedTraitId, setSelectedTraitId] = useState<string | null>(null);
  const [spotlightTargetPieceId, setSpotlightTargetPieceId] = useState<string | null>(null);
  const [draggedPieceId, setDraggedPieceId] = useState<string | null>(null);

  const [showAdvisor, setShowAdvisor] = useState(true);
  const [showInspectDrawer, setShowInspectDrawer] = useState(false);
  const [showNewMatchModal, setShowNewMatchModal] = useState(false);

  const [newDiff, setNewDiff] = useState(120);
  const [newAffixCount, setNewAffixCount] = useState<0 | 1 | 2 | 3 | 4>(2);
  const [newSeed, setNewSeed] = useState('hyacimicon-s2');

  const [battleProgressPct, setBattleProgressPct] = useState(100);
  const [toast, setToast] = useState<{ text: string; isError?: boolean } | null>(null);

  const notify = (text: string, isError = false): void => {
    setToast({ text, isError });
  };

  useEffect(() => {
    if (!toast) {
      return;
    }
    const timer = window.setTimeout(() => setToast(null), 3200);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const refreshInspect = (svc: LocalSimulatedMatchService, matchId: string): void => {
    setInspect(svc.getDebugInspect(matchId));
  };

  const startNewMatch = async (
    svc: LocalSimulatedMatchService,
    difficulty: number,
    affixCount: 0 | 1 | 2 | 3 | 4,
    seed: string,
  ): Promise<void> => {
    const res = await svc.createMatch({
      accountId: 'trial-all',
      rulesVersion: 's2-v4.7',
      enemyDifficulty: difficulty,
      affixCount,
      randomness: seed.trim() ? { kind: 'seeded', seed: seed.trim() } : { kind: 'automatic' },
    });
    if (!res.ok) {
      notify('message' in res.error ? res.error.message : res.error.code, true);
      return;
    }
    setSnapshot(res.value);
    setSelectedPieceId(null);
    setSelectedInvItemId(null);
    setSelectedTraitId(null);
    setSpotlightTargetPieceId(null);
    refreshInspect(svc, res.value.matchId);
  };

  useEffect(() => {
    let cancelled = false;

    loadEncyclopediaCatalog()
      .then(async (loaded) => {
        if (cancelled) {
          return;
        }
        const svc = new LocalSimulatedMatchService(loaded);
        serviceRef.current = svc;
        setCatalog(loaded);
        await startNewMatch(svc, 120, 2, 'hyacimicon-s2');
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setLoadError(err instanceof Error ? err.message : '无法加载图鉴数据');
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const runAction = async (
    legal: LegalAction | undefined,
    successNote?: string,
  ): Promise<void> => {
    const svc = serviceRef.current;
    if (!svc || !snapshot || !legal) {
      return;
    }
    const res = await svc.executeAction({
      matchId: snapshot.matchId,
      revision: snapshot.revision,
      actionId: legal.actionId,
    });
    if (!res.ok) {
      notify(`操作被拒绝: ${res.error.code}`, true);
      return;
    }
    setSnapshot(res.value);
    refreshInspect(svc, res.value.matchId);
    if (successNote) {
      notify(successNote);
    }
  };

  const runAiSingleStep = async (): Promise<void> => {
    const svc = serviceRef.current;
    if (!svc || !snapshot) {
      return;
    }
    const handoff = await svc.setController({
      matchId: snapshot.matchId,
      revision: snapshot.revision,
      controller: { kind: 'model', modelId: 'hyacimicon-heuristic-v1' },
    });
    if (!handoff.ok) {
      notify(`交接失败: ${handoff.error.code}`, true);
      return;
    }

    const recs = svc.suggestActions(handoff.value);
    const chosenId = recs[0]?.id ?? handoff.value.legalActions[0]?.actionId;
    let latest = handoff.value;

    if (chosenId) {
      const actRes = await svc.executeAction(
        {
          matchId: latest.matchId,
          revision: latest.revision,
          actionId: chosenId,
        },
        { kind: 'model', modelId: 'hyacimicon-heuristic-v1' },
      );
      if (actRes.ok) {
        latest = actRes.value;
      }
    }

    const back = await svc.setController({
      matchId: latest.matchId,
      revision: latest.revision,
      controller: { kind: 'manual' },
    });
    if (back.ok) {
      setSnapshot(back.value);
      refreshInspect(svc, back.value.matchId);
      notify(recs[0] ? `AI 托管执行：${recs[0].title}` : 'AI 托管完成一步操作');
    }
  };

  const settleCurrentBattle = async (): Promise<void> => {
    const svc = serviceRef.current;
    if (!svc || !snapshot || snapshot.phase.kind !== 'battle') {
      return;
    }
    const currentNode = snapshot.position;
    const isReward = currentNode?.kind === 'reward';
    const drops: { quality: CrystalQuality; count: number }[] =
      battleProgressPct >= 100
        ? [{ quality: isReward ? 'blue' : 'white', count: 1 }]
        : [];

    const res = await svc.completeBattleAttempt(snapshot.matchId, {
      matchId: snapshot.matchId,
      attemptId: snapshot.phase.attemptId,
      outcome: battleProgressPct >= 100 ? 'won' : 'lost',
      progress:
        battleProgressPct >= 100
          ? { kind: 'complete' }
          : { kind: 'incomplete', percent: battleProgressPct },
      crystalDrops: drops,
    });
    if (!res.ok) {
      notify(`结算失败: ${res.error.code}`, true);
      return;
    }
    setSnapshot(res.value);
    refreshInspect(svc, res.value.matchId);
    setBattleProgressPct(100);
    const s = res.value.latestSettlement;
    if (s) {
      notify(
        `节点 ${s.nodeId} 结算：+${s.baseGold + s.interestGold + s.streakGold} 金，HP ${s.healthBefore}→${s.healthAfter}`,
      );
    }
  };

  const recommendations = useMemo(() => {
    if (!snapshot || !serviceRef.current) {
      return [];
    }
    return serviceRef.current.suggestActions(snapshot);
  }, [snapshot]);

  if (loadError) {
    return (
      <div style={{ padding: '48px 24px', maxWidth: '640px', margin: '0 auto' }}>
        <HsrPanel eyebrow="SIMULATOR INIT" title="运营模拟器初始化失败" accent="gold">
          <p style={{ margin: 0, color: 'var(--hsr-crimson)' }}>{loadError}</p>
        </HsrPanel>
      </div>
    );
  }

  if (!catalog || !snapshot) {
    return (
      <div style={{ padding: '80px 24px', textAlign: 'center', color: 'var(--hsr-muted)' }}>
        正在初始化《崩坏：星穹铁道》货币战争运营模拟器...
      </div>
    );
  }

  const prep = snapshot.preparation;
  const pos = snapshot.position;
  const currentRouteNode: RouteNode = pos ?? { nodeId: '1-1', kind: 'reward' };
  const routeItems: RouteNodeItem[] = snapshot.route.map((node, idx) => ({
    nodeId: node.nodeId,
    plane: node.nodeId.startsWith('1-') ? 1 : node.nodeId.startsWith('2-') ? 2 : 3,
    kind: node.kind,
    status:
      idx < (pos?.index ?? 0)
        ? 'done'
        : idx === (pos?.index ?? 0)
          ? 'current'
          : 'upcoming',
  }));

  const findAction = (
    predicate: (a: LegalAction['action']) => boolean,
  ): LegalAction | undefined => snapshot.legalActions.find((la) => predicate(la.action));

  const movingPieceId = draggedPieceId ?? selectedPieceId;
  const isMoveSlotLegal = (zone: Slot['zone'], index: number): boolean => {
    if (!movingPieceId) {
      return false;
    }
    return Boolean(
      findAction(
        (a) =>
          a.kind === 'move' &&
          a.pieceId === movingPieceId &&
          a.destination.zone === zone &&
          a.destination.index === index,
      ),
    );
  };

  const handleSlotClick = (zone: Slot['zone'], index: number, occupant: Piece | null): void => {
    // Priority 1: If an inventory item is selected and user clicks a piece, try equipping
    if (selectedInvItemId && occupant) {
      const equipAct = findAction(
        (a) =>
          a.kind === 'equip' &&
          a.itemId === selectedInvItemId &&
          a.pieceId === occupant.pieceId,
      );
      if (equipAct) {
        setSelectedInvItemId(null);
        void runAction(equipAct, '已装备到角色');
        return;
      }
    }

    // Priority 2: If a piece is selected for movement, try moving to destination
    if (selectedPieceId && selectedPieceId !== occupant?.pieceId) {
      const moveAct = findAction(
        (a) =>
          a.kind === 'move' &&
          a.pieceId === selectedPieceId &&
          a.destination.zone === zone &&
          a.destination.index === index,
      );
      if (moveAct) {
        setSelectedPieceId(null);
        void runAction(moveAct);
        return;
      }
    }

    // Priority 3: Toggle selection
    if (occupant) {
      setSelectedPieceId((prev) =>
        prev === occupant.pieceId ? null : occupant.pieceId,
      );
    } else {
      setSelectedPieceId(null);
    }
  };

  const selectedPiece = prep?.board.pieces.find((p) => p.pieceId === selectedPieceId) ?? null;
  const selectedPieceSellAct = selectedPiece
    ? findAction((a) => a.kind === 'sell' && a.pieceId === selectedPiece.pieceId)
    : undefined;
  const selectedPieceInfo = selectedPiece
    ? catalog.charactersById[selectedPiece.unitId]
    : undefined;

  const frontSlots = Array.from(
    { length: 4 },
    (_, index) =>
      prep?.board.pieces.find(
        (p) => p.slot.zone === 'front' && p.slot.index === index,
      ) ?? null,
  );
  const backSlots = Array.from(
    { length: 4 },
    (_, index) =>
      prep?.board.pieces.find(
        (p) => p.slot.zone === 'back' && p.slot.index === index,
      ) ?? null,
  );
  const benchSlots = Array.from(
    { length: 9 },
    (_, index) =>
      prep?.board.pieces.find(
        (p) => p.slot.zone === 'bench' && p.slot.index === index,
      ) ?? null,
  );

  const traitCards: ShowcaseTrait[] = (prep?.traits ?? []).map((tp) => {
    const info = catalog.traitsById[tp.traitId];
    const activeTier = info?.tiers.find((t) => t.level === tp.activeThreshold);
    return {
      id: tp.traitId,
      name: info?.name ?? tp.traitId,
      group: info?.group ?? '独立',
      count: tp.count,
      thresholds: tp.thresholds,
      tier: toTraitTier(activeTier?.quality, tp.activeThreshold !== null),
      imagePath: info?.imagePath ?? '',
      description: info?.description ?? '',
    };
  });

  const activeTraitObj = traitCards.find((t) => t.id === selectedTraitId) ?? null;
  const currentOdds = getShopOdds(prep?.resources.level ?? 3);
  const expCurrent = prep?.resources.experience ?? 0;
  const expNeeded = prep?.resources.experienceToNextLevel ?? null;
  const expPct =
    expNeeded && expNeeded > 0 ? Math.min(100, Math.round((expCurrent / expNeeded) * 100)) : 100;

  const interestLit = prep
    ? Math.min(prep.resources.interestCap, Math.floor(prep.resources.gold / 10))
    : 0;

  const isRevisitablePrep =
    (snapshot.phase.kind === 'investmentStrategy' ||
      snapshot.phase.kind === 'supply' ||
      snapshot.phase.kind === 'spotlight') &&
    snapshot.phase.view === 'preparation';

  return (
    <div className="hsr-prep-hud">
      {/* Match Session Control Bar */}
      <div className="hsr-match-bar">
        <div className="hsr-match-meta">
          <HsrTag tone="gold">对局 {snapshot.matchId}</HsrTag>
          <HsrTag tone="violet">版本 rev.{snapshot.revision}</HsrTag>
          <HsrTag tone={snapshot.controller.kind === 'manual' ? 'emerald' : 'gold'}>
            {snapshot.controller.kind === 'manual'
              ? '控制方：玩家手动'
              : `控制方：${snapshot.controller.modelId}`}
          </HsrTag>
          <span style={{ color: 'var(--hsr-muted)' }}>
            已进入触发节点 {pos?.enteredTriggerNodes ?? 0} · 敌方：{snapshot.rival?.name ?? '未知'}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
          <HsrButton
            variant="ghost"
            size="sm"
            onClick={() => setShowNewMatchModal(true)}
          >
            新建对局
          </HsrButton>
          <HsrButton
            variant="secondary"
            size="sm"
            disabled={snapshot.legalActions.length === 0}
            onClick={() => void runAiSingleStep()}
          >
            AI 托管一步
          </HsrButton>
          <HsrButton
            variant={showInspectDrawer ? 'primary' : 'ghost'}
            size="sm"
            onClick={() => setShowInspectDrawer((prev) => !prev)}
          >
            {showInspectDrawer ? '收起检查面板' : '规则检查面板'}
          </HsrButton>
        </div>
      </div>

      {/* Top HUD Bar */}
      <header className="hsr-hud-top">
        <div className="hsr-hud-node-info">
          <span className="hsr-hud-node-badge">
            节点 {currentRouteNode.nodeId}
          </span>
          {snapshot.environment && (
            <HsrTag tone="violet">环境：{snapshot.environment.name}</HsrTag>
          )}
          <div className="hsr-hud-affixes">
            {snapshot.affixes.map((affix) => (
              <span key={affix.id} title={affix.detail ?? ''}>
                <HsrTag tone="crimson">{affix.name}</HsrTag>
              </span>
            ))}
          </div>
        </div>

        <RouteTimeline nodes={routeItems} />

        <div className="hsr-hud-resources">
          <div className="hsr-stat-pill" title="小队生命值">
            <img src={resourceIconUrl('squad-health')} alt="" aria-hidden="true" />
            <strong style={{ color: 'var(--hsr-emerald)' }}>
              {prep?.resources.health ?? 100}
            </strong>
            <span style={{ color: 'var(--hsr-muted)', fontSize: '11px' }}>
              / {prep?.resources.maxHealth ?? 100}
            </span>
          </div>

          <div
            className="hsr-stat-pill"
            title={`当前金币与利息 (${interestLit}/${prep?.resources.interestCap ?? 5})`}
          >
            <img src={COIN_ICON_URL} alt="" aria-hidden="true" />
            <strong style={{ color: 'var(--hsr-gold-bright)' }}>
              {prep?.resources.gold ?? 0}
            </strong>
            <span className="hsr-interest-pips">
              {Array.from({ length: prep?.resources.interestCap ?? 5 }, (_, i) => (
                <span
                  key={i}
                  className={`hsr-interest-pip ${i < interestLit ? 'is-lit' : ''}`}
                />
              ))}
            </span>
          </div>

          <HsrTag tone="gold">🔥 {prep?.resources.hotStreak ?? 0} 连胜</HsrTag>

          <HsrButton
            variant={showAdvisor ? 'primary' : 'secondary'}
            size="sm"
            onClick={() => setShowAdvisor((prev) => !prev)}
          >
            {showAdvisor ? 'AI 参谋 · 开' : 'AI 参谋 · 关'}
          </HsrButton>
        </div>
      </header>

      {/* Main 3-Column Stage */}
      <div className="hsr-hud-stage">
        {/* Left Rail: Traits */}
        <HsrPanel
          eyebrow="SYNERGIES"
          title="激活羁绊"
          rightSlot={
            <HsrTag tone="gold">
              {traitCards.filter((t) => t.tier !== 'inactive').length} 已激活
            </HsrTag>
          }
        >
          <div className="hsr-rail-scroll">
            {traitCards.length === 0 ? (
              <div style={{ fontSize: '12px', color: 'var(--hsr-muted)', padding: '12px 4px' }}>
                部署角色上场后将实时计算羁绊层数。
              </div>
            ) : (
              traitCards.map((trait) => (
                <TraitBadge
                  key={trait.id}
                  trait={trait}
                  active={selectedTraitId === trait.id}
                  onClick={() =>
                    setSelectedTraitId((prev) => (prev === trait.id ? null : trait.id))
                  }
                />
              ))
            )}
          </div>

          {activeTraitObj && (
            <div
              style={{
                marginTop: '10px',
                padding: '10px',
                borderRadius: '3px',
                background: 'rgba(11, 14, 26, 0.86)',
                border: '1px solid var(--hsr-line-gold)',
                fontSize: '12px',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  marginBottom: '6px',
                  fontWeight: 600,
                  color: 'var(--hsr-gold-bright)',
                }}
              >
                <span>{activeTraitObj.name}</span>
                <HsrTag>{activeTraitObj.group}</HsrTag>
              </div>
              <p style={{ margin: 0, lineHeight: 1.65, color: '#dcd9ec' }}>
                <HsrRichText text={activeTraitObj.description} />
              </p>
            </div>
          )}
        </HsrPanel>

        {/* Center: Front / Back Board + Bench */}
        <HsrPanel
          eyebrow="TACTICAL FORMATION"
          title="备战阵型（支持拖拽或点击换位）"
          accent="gold"
          rightSlot={
            <HsrTag tone="emerald">
              上阵 {prep?.board.active ?? 0} / {prep?.resources.level ?? 3}
            </HsrTag>
          }
          bodyClassName="hsr-board-wrap"
        >
          {isRevisitablePrep && (
            <div className="hsr-revisit-banner">
              <span>
                {snapshot.phase.kind === 'spotlight'
                  ? '当前处于【主角光环】返回备战视图：仅开放调整站位（禁止开启晶矿）'
                  : '当前处于待选返回备战视图：仅开放开启晶矿'}
              </span>
              <HsrButton
                variant="primary"
                size="sm"
                onClick={() =>
                  void runAction(findAction((a) => a.kind === 'returnToSelection'))
                }
              >
                返回选择界面
              </HsrButton>
            </div>
          )}

          {snapshot.phase.kind === 'battleAway' && (
            <div className="hsr-revisit-banner">
              <span>战斗暂离中：仅可调整角色站位，重进后将使用新的战斗随机序列</span>
              <HsrButton
                variant="primary"
                size="sm"
                onClick={() =>
                  void runAction(findAction((a) => a.kind === 'reenterBattle'))
                }
              >
                重新进入战斗 ⚔
              </HsrButton>
            </div>
          )}

          {/* Front Stage */}
          <div>
            <div className="hsr-board-zone-head">
              <span>FRONT STAGE · 前台战区 ({prep?.board.front ?? 0} / 4)</span>
              <span>仅支持前台 / 前后台角色</span>
            </div>
            <div className="hsr-board-grid-4">
              {frontSlots.map((piece, idx) => {
                const legalTarget = isMoveSlotLegal('front', idx);
                const illegalTarget = Boolean(movingPieceId && movingPieceId !== piece?.pieceId && !legalTarget);
                const charData = piece
                  ? toShowcaseCharacter(catalog, piece.unitId, piece.stars)
                  : null;
                const equipItems = piece
                  ? piece.equipment.map((eq) => toShowcaseItem(catalog, eq.definitionId))
                  : [];
                const activeAura = piece
                  ? prep?.selectedAuras.find((a) => a.pieceId === piece.pieceId && a.effectActive)
                  : undefined;

                return (
                  <div
                    key={`front-${idx}`}
                    className={`hsr-slot-target ${legalTarget ? 'is-drop-legal' : ''} ${illegalTarget ? 'is-drop-illegal' : ''}`}
                    draggable={Boolean(piece)}
                    onDragStart={() => piece && setDraggedPieceId(piece.pieceId)}
                    onDragEnd={() => setDraggedPieceId(null)}
                    onDragOver={(e) => {
                      if (legalTarget) {
                        e.preventDefault();
                      }
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (draggedPieceId) {
                        const moveAct = findAction(
                          (a) =>
                            a.kind === 'move' &&
                            a.pieceId === draggedPieceId &&
                            a.destination.zone === 'front' &&
                            a.destination.index === idx,
                        );
                        setDraggedPieceId(null);
                        if (moveAct) {
                          void runAction(moveAct);
                        } else {
                          notify('该角色无法放置在此前台格位', true);
                        }
                      }
                    }}
                  >
                    {piece && charData ? (
                      <CharacterCard
                        character={charData}
                        mode="board"
                        equipment={equipItems}
                        spotlightAuraName={activeAura?.name}
                        selected={selectedPieceId === piece.pieceId}
                        onClick={() => handleSlotClick('front', idx, piece)}
                      />
                    ) : (
                      <button
                        type="button"
                        className="hsr-empty-slot hsr-empty-slot--board"
                        style={{ width: '100%' }}
                        onClick={() => handleSlotClick('front', idx, null)}
                      >
                        前台空位 {idx + 1}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          <div className="hsr-board-divider">
            <span>◇ 战线分割 · FRONT / BACK ◇</span>
          </div>

          {/* Back Stage */}
          <div>
            <div className="hsr-board-zone-head">
              <span>BACK STAGE · 后台支援区 ({prep?.board.back ?? 0} / 4)</span>
              <span>仅支持后台 / 前后台角色</span>
            </div>
            <div className="hsr-board-grid-4">
              {backSlots.map((piece, idx) => {
                const legalTarget = isMoveSlotLegal('back', idx);
                const illegalTarget = Boolean(movingPieceId && movingPieceId !== piece?.pieceId && !legalTarget);
                const charData = piece
                  ? toShowcaseCharacter(catalog, piece.unitId, piece.stars)
                  : null;
                const equipItems = piece
                  ? piece.equipment.map((eq) => toShowcaseItem(catalog, eq.definitionId))
                  : [];
                const activeAura = piece
                  ? prep?.selectedAuras.find((a) => a.pieceId === piece.pieceId && a.effectActive)
                  : undefined;

                return (
                  <div
                    key={`back-${idx}`}
                    className={`hsr-slot-target ${legalTarget ? 'is-drop-legal' : ''} ${illegalTarget ? 'is-drop-illegal' : ''}`}
                    draggable={Boolean(piece)}
                    onDragStart={() => piece && setDraggedPieceId(piece.pieceId)}
                    onDragEnd={() => setDraggedPieceId(null)}
                    onDragOver={(e) => {
                      if (legalTarget) {
                        e.preventDefault();
                      }
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (draggedPieceId) {
                        const moveAct = findAction(
                          (a) =>
                            a.kind === 'move' &&
                            a.pieceId === draggedPieceId &&
                            a.destination.zone === 'back' &&
                            a.destination.index === idx,
                        );
                        setDraggedPieceId(null);
                        if (moveAct) {
                          void runAction(moveAct);
                        } else {
                          notify('该角色无法放置在此后台格位', true);
                        }
                      }
                    }}
                  >
                    {piece && charData ? (
                      <CharacterCard
                        character={charData}
                        mode="board"
                        equipment={equipItems}
                        spotlightAuraName={activeAura?.name}
                        selected={selectedPieceId === piece.pieceId}
                        onClick={() => handleSlotClick('back', idx, piece)}
                      />
                    ) : (
                      <button
                        type="button"
                        className="hsr-empty-slot hsr-empty-slot--board"
                        style={{ width: '100%' }}
                        onClick={() => handleSlotClick('back', idx, null)}
                      >
                        后台空位 {idx + 1}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Bench */}
          <div>
            <div className="hsr-board-zone-head">
              <span>BENCH · 备战席 ({prep?.board.bench ?? 0} / 9)</span>
              <span>点击角色后点击目标空位即可移动或交换</span>
            </div>
            <div className="hsr-bench-grid">
              {benchSlots.map((piece, idx) => {
                const legalTarget = isMoveSlotLegal('bench', idx);
                const charData = piece
                  ? toShowcaseCharacter(catalog, piece.unitId, piece.stars)
                  : null;

                return (
                  <div
                    key={`bench-${idx}`}
                    className={`hsr-slot-target ${legalTarget ? 'is-drop-legal' : ''}`}
                    draggable={Boolean(piece)}
                    onDragStart={() => piece && setDraggedPieceId(piece.pieceId)}
                    onDragEnd={() => setDraggedPieceId(null)}
                    onDragOver={(e) => {
                      if (legalTarget) {
                        e.preventDefault();
                      }
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (draggedPieceId) {
                        const moveAct = findAction(
                          (a) =>
                            a.kind === 'move' &&
                            a.pieceId === draggedPieceId &&
                            a.destination.zone === 'bench' &&
                            a.destination.index === idx,
                        );
                        setDraggedPieceId(null);
                        if (moveAct) {
                          void runAction(moveAct);
                        }
                      }
                    }}
                  >
                    {piece && charData ? (
                      <CharacterCard
                        character={charData}
                        mode="bench"
                        selected={selectedPieceId === piece.pieceId}
                        onClick={() => handleSlotClick('bench', idx, piece)}
                      />
                    ) : (
                      <button
                        type="button"
                        className="hsr-empty-slot hsr-empty-slot--bench"
                        style={{ width: '100%' }}
                        onClick={() => handleSlotClick('bench', idx, null)}
                      >
                        {idx + 1}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {selectedPiece && selectedPieceInfo && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: '10px',
                padding: '8px 12px',
                borderRadius: '3px',
                background: 'rgba(11, 14, 26, 0.82)',
                border: '1px solid var(--hsr-line-gold)',
                flexWrap: 'wrap',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                <strong style={{ color: 'var(--hsr-gold-bright)' }}>
                  {selectedPieceInfo.name} ({selectedPiece.stars}★)
                </strong>
                <CostBadge cost={selectedPieceInfo.cost} />
                <PositionBadge position={selectedPieceInfo.position} />
                {selectedPieceInfo.traits.map((t) => (
                  <HsrTag key={t} tone="violet">
                    {t}
                  </HsrTag>
                ))}
              </div>
              <div style={{ display: 'flex', gap: '8px' }}>
                <HsrButton
                  variant="danger"
                  size="sm"
                  disabled={!selectedPieceSellAct}
                  onClick={() => {
                    setSelectedPieceId(null);
                    void runAction(selectedPieceSellAct);
                  }}
                >
                  出售 (+{sellGoldValue(selectedPieceInfo.cost, selectedPiece.stars)} 金)
                </HsrButton>
              </div>
            </div>
          )}
        </HsrPanel>

        {/* Right Rail: Inventory, Crystals & Live AI Advisor */}
        <div className="hsr-right-rail">
          <HsrPanel
            eyebrow="INVENTORY & CRYSTALS"
            title="装备栏与晶矿"
            rightSlot={<HsrTag>{prep?.inventory.length ?? 0} 件</HsrTag>}
          >
            {prep && prep.inventory.length === 0 ? (
              <div style={{ fontSize: '12px', color: 'var(--hsr-muted)', padding: '6px 0' }}>
                暂无装备，可开启晶矿或在补给节点获取。
              </div>
            ) : (
              <div className="hsr-inv-grid">
                {(prep?.inventory ?? []).map((stack) => {
                  const info = toShowcaseItem(catalog, stack.definitionId, stack.count);
                  const isSel = selectedInvItemId === stack.itemId;
                  const canCombineWithSelected =
                    Boolean(selectedInvItemId) &&
                    Boolean(
                      findAction(
                        (a) =>
                          a.kind === 'combine' &&
                          ((a.itemId === selectedInvItemId && a.otherItemId === stack.itemId) ||
                            (a.otherItemId === selectedInvItemId && a.itemId === stack.itemId)),
                      ),
                    );

                  return (
                    <button
                      key={stack.itemId}
                      type="button"
                      className={`hsr-inv-item-btn ${isSel ? 'is-selected' : ''} ${canCombineWithSelected ? 'is-combinable' : ''}`}
                      onClick={() => {
                        if (selectedInvItemId && canCombineWithSelected) {
                          const combineAct = findAction(
                            (a) =>
                              a.kind === 'combine' &&
                              ((a.itemId === selectedInvItemId &&
                                a.otherItemId === stack.itemId) ||
                                (a.otherItemId === selectedInvItemId &&
                                  a.itemId === stack.itemId)),
                          );
                          setSelectedInvItemId(null);
                          void runAction(combineAct, '已合成进阶装备');
                        } else {
                          setSelectedInvItemId((prev) =>
                            prev === stack.itemId ? null : stack.itemId,
                          );
                        }
                      }}
                    >
                      <ItemFrameIcon
                        imagePath={info.imagePath}
                        frame={info.frame}
                        name={`${info.name}：${info.description}`}
                        size="md"
                        count={stack.count}
                      />
                    </button>
                  );
                })}
              </div>
            )}

            {selectedInvItemId && (
              <div style={{ marginTop: '8px', fontSize: '11px', color: 'var(--hsr-gold)' }}>
                已选中装备：点击棋盘角色即可穿戴，或点击绿色高亮材料进行合成。
              </div>
            )}

            <div className="hsr-crystal-grid" style={{ marginTop: '10px' }}>
              {(prep?.crystals ?? []).map((cry) => {
                const openAct = findAction(
                  (a) => a.kind === 'openCrystal' && a.crystalId === cry.crystalId,
                );
                return (
                  <CrystalBadge
                    key={cry.crystalId}
                    quality={cry.quality}
                    label={CRYSTAL_LABELS[cry.quality]}
                    count={cry.count}
                    onClick={() => {
                      if (!openAct) {
                        notify(
                          prep && prep.board.bench >= 9
                            ? '备战席已满 9 格，禁止开启晶矿'
                            : '当前阶段不允许开启晶矿',
                          true,
                        );
                        return;
                      }
                      void runAction(openAct, `已开启${CRYSTAL_LABELS[cry.quality]}`);
                    }}
                  />
                );
              })}
            </div>
          </HsrPanel>

          {showAdvisor && (
            <HsrPanel
              eyebrow="HYACIMICON ADVISOR"
              title="AI 实时策略参谋"
              accent="violet"
              rightSlot={<HsrTag tone="emerald">实时合法动作</HsrTag>}
            >
              {recommendations.length === 0 ? (
                <div style={{ fontSize: '12px', color: 'var(--hsr-muted)' }}>
                  当前暂无需要特别提示的操作，可点击「开始战斗」推进节点。
                </div>
              ) : (
                recommendations.map((rec) => {
                  const act = snapshot.legalActions.find((la) => la.actionId === rec.id);
                  return (
                    <div key={rec.id} className="hsr-ai-card">
                      <div className="hsr-ai-card-top">
                        <span className="hsr-ai-card-title">{rec.title}</span>
                        <HsrTag tone="emerald">{rec.sssDelta}</HsrTag>
                      </div>
                      <div
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'space-between',
                          gap: '6px',
                          marginBottom: '4px',
                        }}
                      >
                        <div style={{ display: 'flex', gap: '6px' }}>
                          <HsrTag tone="violet">{rec.tag}</HsrTag>
                          {rec.goldCost !== undefined && <CostBadge cost={rec.goldCost} />}
                        </div>
                        <HsrButton
                          variant="secondary"
                          size="sm"
                          disabled={!act}
                          onClick={() => void runAction(act)}
                        >
                          执行
                        </HsrButton>
                      </div>
                      <p className="hsr-ai-card-desc">
                        <HsrRichText text={rec.rationale} />
                      </p>
                    </div>
                  );
                })
              )}
            </HsrPanel>
          )}
        </div>
      </div>

      {/* Bottom Economy & Shop Dock */}
      <footer className="hsr-shop-dock">
        <div className="hsr-shop-exp-panel">
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <img
                src={resourceIconUrl('experience')}
                alt=""
                aria-hidden="true"
                style={{ width: '18px', height: '18px' }}
              />
              <strong style={{ fontSize: '14px', color: 'var(--hsr-gold-bright)' }}>
                商店 Lv.{prep?.resources.level ?? 3}
              </strong>
            </div>
            <span style={{ fontSize: '11px', color: 'var(--hsr-muted)' }}>
              {expNeeded === null ? '已满级' : `${expCurrent} / ${expNeeded} EXP`}
            </span>
          </div>

          <div className="hsr-exp-track">
            <div className="hsr-exp-fill" style={{ width: `${expPct}%` }} />
            <span className="hsr-exp-label">{expPct}%</span>
          </div>

          <div className="hsr-odds-row">
            {([1, 2, 3, 4, 5] as const).map((fee) => (
              <span key={fee}>
                {fee}费 <strong style={{ color: 'var(--hsr-ink)' }}>{currentOdds[fee]}%</strong>
              </span>
            ))}
          </div>

          <div style={{ display: 'flex', gap: '6px' }}>
            <HsrButton
              variant="secondary"
              size="sm"
              cost={4}
              disabled={!findAction((a) => a.kind === 'buyExperience')}
              onClick={() =>
                void runAction(findAction((a) => a.kind === 'buyExperience'))
              }
              style={{ flex: 1 }}
            >
              购买经验
            </HsrButton>
            <HsrButton
              variant="secondary"
              size="sm"
              cost={prep && prep.shop.freeRefreshes > 0 ? 0 : 2}
              disabled={!findAction((a) => a.kind === 'refreshShop')}
              onClick={() =>
                void runAction(findAction((a) => a.kind === 'refreshShop'))
              }
              style={{ flex: 1 }}
            >
              {prep && prep.shop.freeRefreshes > 0
                ? `免费刷新 (${prep.shop.freeRefreshes})`
                : '刷新商店'}
            </HsrButton>
          </div>
        </div>

        <div className="hsr-shop-offers">
          {(prep?.shop.slots ?? [null, null, null, null, null]).map((offer, idx) => {
            if (!offer) {
              return (
                <div key={`shop-empty-${idx}`} className="hsr-empty-slot hsr-empty-slot--board">
                  已售罄
                </div>
              );
            }
            const buyAct = findAction(
              (a) => a.kind === 'buy' && a.offerId === offer.offerId,
            );
            const ownedCount =
              prep?.board.pieces.filter((p) => p.unitId === offer.unitId).length ?? 0;

            return (
              <CharacterCard
                key={offer.offerId}
                character={toShowcaseCharacter(catalog, offer.unitId, 1)}
                mode="shop"
                price={offer.price}
                upgradeable={ownedCount >= 2}
                onClick={() => {
                  if (!buyAct) {
                    notify('金币不足或备战席已满', true);
                    return;
                  }
                  void runAction(buyAct);
                }}
              />
            );
          })}
        </div>

        <div className="hsr-shop-actions-col">
          <div style={{ display: 'flex', gap: '6px' }}>
            <HsrButton
              variant={prep?.shop.locked ? 'primary' : 'ghost'}
              size="sm"
              disabled={!findAction((a) => a.kind === 'lockShop')}
              onClick={() => void runAction(findAction((a) => a.kind === 'lockShop'))}
              style={{ flex: 1 }}
            >
              {prep?.shop.locked ? '已锁定商店' : '锁定商店'}
            </HsrButton>
            {snapshot.phase.kind === 'preparation' &&
              snapshot.phase.spotlightAvailable && (
                <HsrButton
                  variant="secondary"
                  size="sm"
                  onClick={() =>
                    void runAction(findAction((a) => a.kind === 'openSpotlight'))
                  }
                  style={{ flex: 1 }}
                >
                  ✦ 主角光环
                </HsrButton>
              )}
          </div>

          {snapshot.latestSettlement && (
            <div
              style={{
                padding: '6px 9px',
                borderRadius: '3px',
                background: 'rgba(13, 17, 32, 0.78)',
                border: '1px solid var(--hsr-line)',
                fontSize: '11px',
                color: 'var(--hsr-muted)',
              }}
            >
              上局 {snapshot.latestSettlement.nodeId}：+
              {snapshot.latestSettlement.baseGold +
                snapshot.latestSettlement.interestGold +
                snapshot.latestSettlement.streakGold}{' '}
              金 · HP {snapshot.latestSettlement.healthBefore}→
              {snapshot.latestSettlement.healthAfter}
            </div>
          )}

          <HsrButton
            variant="primary"
            size="lg"
            disabled={!findAction((a) => a.kind === 'startBattle')}
            onClick={() => void runAction(findAction((a) => a.kind === 'startBattle'))}
            style={{ width: '100%' }}
          >
            开始战斗 ⚔
          </HsrButton>
        </div>
      </footer>

      {/* Active Phase Modal Overlays */}
      {snapshot.phase.kind === 'investmentEnvironment' && (
        <div className="hsr-modal-overlay">
          <h2 className="hsr-overlay-title">选择本局投资环境</h2>
          <p className="hsr-overlay-subtitle">
            对局 {snapshot.matchId} · 每个候选具备 1 次刷新机会，选定后进入 1-1
          </p>
          <div className="hsr-cards-row">
            {snapshot.phase.options.map((opt) => {
              const env = catalog.environmentsById[opt.id];
              const refreshAct = findAction(
                (a) => a.kind === 'refreshChoice' && a.optionId === opt.optionId,
              );
              const chooseAct = findAction(
                (a) =>
                  a.kind === 'chooseInvestmentEnvironment' && a.optionId === opt.optionId,
              );
              return (
                <InvestmentCard
                  key={opt.optionId}
                  kind="environment"
                  title={opt.name}
                  description={opt.detail ?? env?.description ?? ''}
                  imagePath={env?.imagePath ?? ''}
                  refreshAvailable={Boolean(refreshAct)}
                  onRefresh={() => void runAction(refreshAct)}
                  onSelect={() => void runAction(chooseAct)}
                />
              );
            })}
          </div>
        </div>
      )}

      {snapshot.phase.kind === 'investmentStrategy' &&
        snapshot.phase.view === 'selection' && (
          <div className="hsr-modal-overlay">
            <h2 className="hsr-overlay-title">
              选择投资策略 · 节点 {currentRouteNode.nodeId}
            </h2>
            <p className="hsr-overlay-subtitle">
              选定策略后正式进入该战斗节点 · 返回备战仅允许开启晶矿
            </p>
            <div className="hsr-cards-row">
              {snapshot.phase.options.map((opt) => {
                const st = catalog.strategiesById[opt.id];
                const refreshAct = findAction(
                  (a) => a.kind === 'refreshChoice' && a.optionId === opt.optionId,
                );
                const chooseAct = findAction(
                  (a) =>
                    a.kind === 'chooseInvestmentStrategy' && a.optionId === opt.optionId,
                );
                return (
                  <InvestmentCard
                    key={opt.optionId}
                    kind="strategy"
                    title={opt.name}
                    quality={opt.quality}
                    qualityLabel={st?.qualityLabel}
                    description={opt.detail ?? st?.description ?? ''}
                    imagePath={st?.imagePath ?? ''}
                    refreshAvailable={Boolean(refreshAct)}
                    onRefresh={() => void runAction(refreshAct)}
                    onSelect={() => void runAction(chooseAct)}
                  />
                );
              })}
            </div>
            <div style={{ marginTop: '18px' }}>
              <HsrButton
                variant="secondary"
                size="md"
                onClick={() =>
                  void runAction(findAction((a) => a.kind === 'returnToPreparation'))
                }
              >
                返回备战开启晶矿
              </HsrButton>
            </div>
          </div>
        )}

      {snapshot.phase.kind === 'supply' && snapshot.phase.view === 'selection' && (
        <div className="hsr-modal-overlay">
          <h2 className="hsr-overlay-title">补给节点 · {currentRouteNode.nodeId}</h2>
          <p className="hsr-overlay-subtitle">
            五组「角色 + 装备」选一，领取后自动推进至下一节点
          </p>
          <div className="hsr-supply-grid">
            {snapshot.phase.options.map((opt) => {
              const takeAct = findAction(
                (a) => a.kind === 'takeSupply' && a.optionId === opt.optionId,
              );
              const char = toShowcaseCharacter(catalog, opt.unitId, 1);
              return (
                <div key={opt.optionId} className="hsr-supply-card">
                  <CharacterCard
                    character={char}
                    mode="gallery"
                    onClick={() => void runAction(takeAct)}
                  />
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                    {opt.itemDefinitionIds.map((defId) => {
                      const item = toShowcaseItem(catalog, defId);
                      return (
                        <div
                          key={defId}
                          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
                        >
                          <ItemFrameIcon
                            imagePath={item.imagePath}
                            frame={item.frame}
                            name={item.name}
                            size="sm"
                          />
                          <span style={{ fontSize: '12px' }}>{item.name}</span>
                        </div>
                      );
                    })}
                  </div>
                  <HsrButton
                    variant="primary"
                    size="sm"
                    onClick={() => void runAction(takeAct)}
                    style={{ width: '100%' }}
                  >
                    领取此补给
                  </HsrButton>
                </div>
              );
            })}
          </div>
          <div style={{ display: 'flex', gap: '12px', marginTop: '18px' }}>
            <HsrButton
              variant="secondary"
              size="md"
              onClick={() =>
                void runAction(findAction((a) => a.kind === 'returnToPreparation'))
              }
            >
              返回备战开启晶矿
            </HsrButton>
            <HsrButton
              variant="secondary"
              size="md"
              disabled={!findAction((a) => a.kind === 'refreshSupply')}
              onClick={() =>
                void runAction(findAction((a) => a.kind === 'refreshSupply'))
              }
            >
              {snapshot.phase.refreshAvailable ? '刷新全部候选 (1/1)' : '已刷新'}
            </HsrButton>
          </div>
        </div>
      )}

      {snapshot.phase.kind === 'spotlight' && snapshot.phase.view === 'selection' && (() => {
        const spotlightPhase = snapshot.phase;
        const resolvedTargetId =
          spotlightTargetPieceId &&
          spotlightPhase.eligiblePieceIds.includes(spotlightTargetPieceId)
            ? spotlightTargetPieceId
            : (spotlightPhase.eligiblePieceIds[0] ?? '');
        return (
        <div className="hsr-modal-overlay">
          <HsrPanel
            eyebrow="SPOTLIGHT AURA"
            title={`主角光环选择 · 首领战 ${currentRouteNode.nodeId}`}
            accent="gold"
            className="hsr-panel--gold"
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', maxWidth: '640px' }}>
              <div style={{ fontSize: '12px', color: 'var(--hsr-muted)' }}>
                1. 选择受光环加成的上阵角色：
              </div>
              <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                {spotlightPhase.eligiblePieceIds.map((pid) => {
                  const piece = prep?.board.pieces.find((p) => p.pieceId === pid);
                  const name = piece
                    ? (catalog.charactersById[piece.unitId]?.name ?? piece.unitId)
                    : pid;
                  const active = resolvedTargetId === pid;
                  return (
                    <button
                      key={pid}
                      type="button"
                      className={`hsr-subtab ${active ? 'is-active' : ''}`}
                      onClick={() => setSpotlightTargetPieceId(pid)}
                    >
                      {name}
                    </button>
                  );
                })}
              </div>

              <div style={{ fontSize: '12px', color: 'var(--hsr-muted)' }}>
                2. 选择或刷新光环（当前刷新价：{spotlightPhase.refreshPrice} 金）：
              </div>
              {spotlightPhase.options.map((opt) => {
                const chooseAct = findAction(
                  (a) =>
                    a.kind === 'chooseSpotlight' &&
                    a.optionId === opt.optionId &&
                    a.pieceId === resolvedTargetId,
                );
                const refreshAct = findAction(
                  (a) => a.kind === 'refreshChoice' && a.optionId === opt.optionId,
                );
                return (
                  <div
                    key={opt.optionId}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      gap: '12px',
                      padding: '10px 12px',
                      borderRadius: '3px',
                      background: 'rgba(14, 18, 33, 0.85)',
                      border: '1px solid var(--hsr-line-gold)',
                    }}
                  >
                    <div style={{ flex: 1 }}>
                      <strong style={{ color: 'var(--hsr-gold-bright)' }}>{opt.name}</strong>
                      <p style={{ margin: '4px 0 0', fontSize: '12px', color: '#dcd9ec' }}>
                        <HsrRichText text={opt.detail ?? ''} />
                      </p>
                    </div>
                    <div style={{ display: 'flex', gap: '6px' }}>
                      <HsrButton
                        variant="ghost"
                        size="sm"
                        cost={spotlightPhase.refreshPrice}
                        disabled={!refreshAct}
                        onClick={() => void runAction(refreshAct)}
                      >
                        刷新
                      </HsrButton>
                      <HsrButton
                        variant="primary"
                        size="sm"
                        disabled={!chooseAct}
                        onClick={() => void runAction(chooseAct)}
                      >
                        赋予
                      </HsrButton>
                    </div>
                  </div>
                );
              })}

              <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                <HsrButton
                  variant="secondary"
                  size="sm"
                  onClick={() =>
                    void runAction(findAction((a) => a.kind === 'returnToPreparation'))
                  }
                >
                  返回备战调整站位
                </HsrButton>
              </div>
            </div>
          </HsrPanel>
        </div>
        );
      })()}

      {snapshot.phase.kind === 'battle' && (
        <div className="hsr-modal-overlay">
          <HsrPanel
            eyebrow="BATTLE HANDOFF"
            title={`节点 ${currentRouteNode.nodeId} · 战斗进行中`}
            accent="gold"
          >
            {(() => {
              const preview = calculateBattleHealthSettlement({
                nodeId: currentRouteNode.nodeId,
                kind: currentRouteNode.kind,
                actualProgress: battleProgressPct,
                healthBefore: prep?.resources.health ?? 100,
                maxHealth: prep?.resources.maxHealth ?? 100,
              });
              return (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', minWidth: '420px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px' }}>
                    <span>
                      我方估算战力：<strong style={{ color: 'var(--hsr-emerald)' }}>{inspect?.estimatedTeamPower ?? 0}</strong>
                    </span>
                    <span>
                      节点推荐战力：<strong style={{ color: 'var(--hsr-gold)' }}>{inspect?.estimatedNodePower ?? 0}</strong>
                    </span>
                  </div>

                  <div>
                    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px', fontSize: '13px' }}>
                      <span>模拟战斗进度</span>
                      <strong style={{ color: battleProgressPct >= 100 ? 'var(--hsr-emerald)' : 'var(--hsr-crimson)' }}>
                        {battleProgressPct}% {battleProgressPct >= 100 ? '（完胜）' : '（未打满）'}
                      </strong>
                    </div>
                    <input
                      type="range"
                      min={0}
                      max={100}
                      value={battleProgressPct}
                      onChange={(e) => setBattleProgressPct(Number(e.target.value))}
                      style={{ width: '100%', accentColor: 'var(--hsr-gold)' }}
                    />
                  </div>

                  <div
                    style={{
                      padding: '10px',
                      borderRadius: '3px',
                      background: 'rgba(11, 14, 26, 0.82)',
                      border: '1px solid var(--hsr-line)',
                      fontSize: '12px',
                    }}
                  >
                    <div className="hsr-stat-row">
                      <span>基础失血 / 进度失血</span>
                      <strong>
                        -{preview.baseHealthLost} / -{preview.progressHealthLost}
                      </strong>
                    </div>
                    <div className="hsr-stat-row">
                      <span>固定回复 / 预计结算血量</span>
                      <strong>
                        +{preview.fixedRecovery} → {preview.healthAfter}
                      </strong>
                    </div>
                  </div>

                  <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                    <HsrButton
                      variant="ghost"
                      size="sm"
                      onClick={() =>
                        void runAction(
                          findAction((a) => a.kind === 'restartBattle'),
                          '直接重开：沿用本次战斗随机序列',
                        )
                      }
                    >
                      直接重开 (同序列)
                    </HsrButton>
                    <HsrButton
                      variant="danger"
                      size="sm"
                      onClick={() =>
                        void runAction(findAction((a) => a.kind === 'leaveBattle'))
                      }
                    >
                      主动暂离 (调站位)
                    </HsrButton>
                    <HsrButton
                      variant="primary"
                      size="md"
                      onClick={() => void settleCurrentBattle()}
                      style={{ marginLeft: 'auto' }}
                    >
                      确认战斗结算
                    </HsrButton>
                  </div>
                </div>
              );
            })()}
          </HsrPanel>
        </div>
      )}

      {snapshot.phase.kind === 'finished' && (
        <div className="hsr-modal-overlay">
          <HsrPanel
            eyebrow="MATCH FINISHED"
            title={
              snapshot.phase.reason === 'cleared'
                ? '✦ 整局通关 · CURRENCY WARS CLEARED'
                : '小队血量耗尽 · DEFEAT'
            }
            accent={snapshot.phase.reason === 'cleared' ? 'gold' : 'violet'}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', minWidth: '340px' }}>
              <div className="hsr-stat-row">
                <span>最终血量</span>
                <strong>
                  {prep?.resources.health ?? 0} / {prep?.resources.maxHealth ?? 100}
                </strong>
              </div>
              <div className="hsr-stat-row">
                <span>最终金币 / 等级</span>
                <strong>
                  {prep?.resources.gold ?? 0} 金 · Lv.{prep?.resources.level ?? 3}
                </strong>
              </div>
              <HsrButton
                variant="primary"
                size="lg"
                onClick={() =>
                  serviceRef.current &&
                  void startNewMatch(serviceRef.current, newDiff, newAffixCount, newSeed)
                }
              >
                再来一局
              </HsrButton>
            </div>
          </HsrPanel>
        </div>
      )}

      {/* New Match Configuration Modal */}
      {showNewMatchModal && (
        <div className="hsr-modal-overlay">
          <HsrPanel eyebrow="CREATE MATCH" title="新建模拟对局参数" accent="gold">
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', minWidth: '340px' }}>
              <label style={{ fontSize: '12px' }}>
                敌人难度 (0 – 299 整数)
                <input
                  type="number"
                  min={0}
                  max={299}
                  value={newDiff}
                  onChange={(e) => setNewDiff(Number(e.target.value))}
                  className="hsr-form-input"
                  style={{ marginTop: '4px' }}
                />
              </label>

              <label style={{ fontSize: '12px' }}>
                负面词缀数量 (0 – 4)
                <select
                  value={newAffixCount}
                  onChange={(e) => {
                    const v = Number(e.target.value);
                    if (v === 0 || v === 1 || v === 2 || v === 3 || v === 4) {
                      setNewAffixCount(v);
                    }
                  }}
                  className="hsr-form-input"
                  style={{ marginTop: '4px' }}
                >
                  <option value={0}>0 个词缀</option>
                  <option value={1}>1 个词缀（强化位）</option>
                  <option value={2}>2 个词缀</option>
                  <option value={3}>3 个词缀</option>
                  <option value={4}>4 个词缀</option>
                </select>
              </label>

              <label style={{ fontSize: '12px' }}>
                随机种子（留空则自动生成）
                <input
                  type="text"
                  value={newSeed}
                  onChange={(e) => setNewSeed(e.target.value)}
                  className="hsr-form-input"
                  style={{ marginTop: '4px' }}
                />
              </label>

              <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
                <HsrButton
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowNewMatchModal(false)}
                >
                  取消
                </HsrButton>
                <HsrButton
                  variant="primary"
                  size="sm"
                  onClick={() => {
                    setShowNewMatchModal(false);
                    if (serviceRef.current) {
                      void startNewMatch(
                        serviceRef.current,
                        newDiff,
                        newAffixCount,
                        newSeed,
                      );
                    }
                  }}
                >
                  创建对局
                </HsrButton>
              </div>
            </div>
          </HsrPanel>
        </div>
      )}

      {/* Rules & Pool Inspector Drawer */}
      {showInspectDrawer && inspect && (
        <HsrPanel
          eyebrow="RULE & STATE INSPECTOR"
          title="运营规则检查面板（非局内可见信息）"
          accent="violet"
          rightSlot={
            <HsrButton
              variant="circle-close"
              onClick={() => setShowInspectDrawer(false)}
              aria-label="关闭"
            >
              ✕
            </HsrButton>
          }
        >
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: '14px' }}>
            <div>
              <div style={{ fontSize: '12px', color: 'var(--hsr-gold)', marginBottom: '6px' }}>
                随机种子与战力估算
              </div>
              <div className="hsr-stat-row">
                <span>对局 Seed</span>
                <strong>{inspect.seed}</strong>
              </div>
              <div className="hsr-stat-row">
                <span>剩余牌库 (1–5费)</span>
                <strong>
                  {[1, 2, 3, 4, 5]
                    .map((c) => `${c}费:${inspect.poolRemainingByCost[c] ?? 0}`)
                    .join(' ')}
                </strong>
              </div>
              <div className="hsr-stat-row">
                <span>合法动作数</span>
                <strong>{snapshot.legalActions.length}</strong>
              </div>
            </div>

            <div>
              <div style={{ fontSize: '12px', color: 'var(--hsr-gold)', marginBottom: '6px' }}>
                最近操作记录
              </div>
              <div style={{ maxHeight: '120px', overflowY: 'auto', fontSize: '11px' }}>
                {inspect.actionLog.map((entry) => (
                  <div
                    key={entry.revision}
                    style={{
                      padding: '3px 0',
                      borderBottom: '1px solid var(--hsr-line)',
                      color: 'var(--hsr-muted)',
                    }}
                  >
                    <strong style={{ color: 'var(--hsr-ink)' }}>v{entry.revision}</strong> ·{' '}
                    {entry.summary}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </HsrPanel>
      )}

      {toast && (
        <div className={`hsr-toast ${toast.isError ? 'hsr-toast--error' : ''}`}>
          {toast.text}
        </div>
      )}
    </div>
  );
}
