import { useState } from 'react';
import { toHsrAssetUrl } from '../assets/paths';
import { CharacterCard } from '../components/domain/CharacterCard';
import { CrystalBadge } from '../components/domain/CrystalBadge';
import { InvestmentCard } from '../components/domain/InvestmentCard';
import { ItemFrameIcon } from '../components/domain/ItemFrameIcon';
import { HsrAssetImage } from '../components/primitives/HsrAssetImage';
import { HsrTag } from '../components/primitives/HsrBadge';
import { HsrButton } from '../components/primitives/HsrButton';
import { HsrPanel } from '../components/primitives/HsrPanel';
import { HsrRichText } from '../components/primitives/HsrRichText';
import {
  AURAS_FIXTURE,
  ENVIRONMENTS_FIXTURE,
  SETTLEMENT_FIXTURE,
  STRATEGIES_FIXTURE,
  SUPPLY_COMBOS_FIXTURE,
} from '../data/showcaseFixtures';

type ModalScene = 'environment' | 'strategy' | 'supply' | 'spotlight';

export function ModalGalleryView() {
  const [scene, setScene] = useState<ModalScene>('strategy');
  const [selectedEnvId, setSelectedEnvId] = useState<string>(ENVIRONMENTS_FIXTURE[2]?.id ?? '');
  const [selectedStrategyId, setSelectedStrategyId] = useState<string>(
    STRATEGIES_FIXTURE[2]?.id ?? '',
  );
  const [selectedSupplyId, setSelectedSupplyId] = useState<string>(
    SUPPLY_COMBOS_FIXTURE[0]?.optionId ?? '',
  );
  const [selectedAuraId, setSelectedAuraId] = useState<string>(AURAS_FIXTURE[0]?.id ?? '');
  const [refreshedIds, setRefreshedIds] = useState<Record<string, boolean>>({});

  const markRefreshed = (id: string): void => {
    setRefreshedIds((prev) => ({ ...prev, [id]: true }));
  };

  return (
    <div className="hsr-modal-gallery">
      <div className="hsr-subtabs" role="tablist" aria-label="对局弹窗样板切换">
        <button
          type="button"
          className={`hsr-subtab ${scene === 'environment' ? 'is-active' : ''}`}
          onClick={() => setScene('environment')}
        >
          01 · 投资环境选择 (开局)
        </button>
        <button
          type="button"
          className={`hsr-subtab ${scene === 'strategy' ? 'is-active' : ''}`}
          onClick={() => setScene('strategy')}
        >
          02 · 投资策略选择 (白银 / 黄金 / 棱彩)
        </button>
        <button
          type="button"
          className={`hsr-subtab ${scene === 'supply' ? 'is-active' : ''}`}
          onClick={() => setScene('supply')}
        >
          03 · 补给节点 (5 选 1 组合)
        </button>
        <button
          type="button"
          className={`hsr-subtab ${scene === 'spotlight' ? 'is-active' : ''}`}
          onClick={() => setScene('spotlight')}
        >
          04 · 主角光环 &amp; 战斗结算报告
        </button>
      </div>

      <div className="hsr-backdrop-stage">
        {scene === 'environment' && (
          <>
            <h2 className="hsr-overlay-title">选择本局投资环境</h2>
            <p className="hsr-overlay-subtitle">
              开局三选一 · 每个候选具备 1 次独立刷新机会
            </p>
            <div className="hsr-cards-row">
              {ENVIRONMENTS_FIXTURE.map((env) => (
                <InvestmentCard
                  key={env.id}
                  kind="environment"
                  title={env.name}
                  description={env.description}
                  imagePath={env.imagePath}
                  selected={selectedEnvId === env.id}
                  refreshAvailable={Boolean(env.refreshAvailable) && !refreshedIds[env.id]}
                  onSelect={() => setSelectedEnvId(env.id)}
                  onRefresh={() => markRefreshed(env.id)}
                />
              ))}
            </div>
            <div style={{ marginTop: '22px' }}>
              <HsrButton variant="primary" size="lg">
                确认环境并进入 1-1
              </HsrButton>
            </div>
          </>
        )}

        {scene === 'strategy' && (
          <>
            <h2 className="hsr-overlay-title">选择投资策略 · 节点 2-2</h2>
            <p className="hsr-overlay-subtitle">
              同时展示白银 / 黄金 / 棱彩三档 SVG 底板与渐变遮罩质感
            </p>
            <div className="hsr-cards-row">
              {STRATEGIES_FIXTURE.map((st) => (
                <InvestmentCard
                  key={st.id}
                  kind="strategy"
                  title={st.name}
                  quality={st.quality}
                  qualityLabel={st.qualityLabel}
                  description={st.description}
                  imagePath={st.imagePath}
                  selected={selectedStrategyId === st.id}
                  refreshAvailable={Boolean(st.refreshAvailable) && !refreshedIds[st.id]}
                  onSelect={() => setSelectedStrategyId(st.id)}
                  onRefresh={() => markRefreshed(st.id)}
                />
              ))}
            </div>
            <div style={{ display: 'flex', gap: '12px', marginTop: '22px' }}>
              <HsrButton variant="secondary" size="md">
                返回备战开启晶矿
              </HsrButton>
              <HsrButton variant="primary" size="md">
                确认选定策略
              </HsrButton>
            </div>
          </>
        )}

        {scene === 'supply' && (
          <>
            <h2 className="hsr-overlay-title">补给节点 · 2-3</h2>
            <p className="hsr-overlay-subtitle">
              五组「角色 + 装备」组合 · 仅限 1 次全部刷新机会
            </p>
            <div className="hsr-supply-grid">
              {SUPPLY_COMBOS_FIXTURE.map((combo) => {
                const isSelected = selectedSupplyId === combo.optionId;
                return (
                  <div
                    key={combo.optionId}
                    className="hsr-supply-card"
                    style={{
                      borderColor: isSelected ? 'var(--hsr-gold-bright)' : undefined,
                      boxShadow: isSelected ? 'var(--hsr-glow-gold)' : undefined,
                    }}
                  >
                    <CharacterCard
                      character={combo.character}
                      mode="gallery"
                      selected={isSelected}
                      onClick={() => setSelectedSupplyId(combo.optionId)}
                    />
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      {combo.items.map((item) => (
                        <div
                          key={item.id}
                          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
                        >
                          <ItemFrameIcon
                            imagePath={item.imagePath}
                            frame={item.frame}
                            name={item.name}
                            size="sm"
                          />
                          <span style={{ fontSize: '12px', color: 'var(--hsr-ink)' }}>
                            {item.name}
                          </span>
                        </div>
                      ))}
                    </div>
                    <HsrButton
                      variant={isSelected ? 'primary' : 'secondary'}
                      size="sm"
                      onClick={() => setSelectedSupplyId(combo.optionId)}
                      style={{ width: '100%' }}
                    >
                      {isSelected ? '已选中' : '领取此补给'}
                    </HsrButton>
                  </div>
                );
              })}
            </div>
            <div style={{ display: 'flex', gap: '12px', marginTop: '20px' }}>
              <HsrButton variant="secondary" size="md">
                返回备战
              </HsrButton>
              <HsrButton variant="secondary" size="md">
                刷新全部候选 (1/1)
              </HsrButton>
            </div>
          </>
        )}

        {scene === 'spotlight' && (
          <div className="hsr-settlement-grid">
            <HsrPanel
              eyebrow="SPOTLIGHT AURA"
              title="主角光环二选一"
              accent="gold"
              rightSlot={<HsrTag tone="gold">目标：阿格莱雅</HsrTag>}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                {AURAS_FIXTURE.map((aura) => {
                  const active = selectedAuraId === aura.id;
                  return (
                    <button
                      key={aura.id}
                      type="button"
                      onClick={() => setSelectedAuraId(aura.id)}
                      style={{
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: '12px',
                        padding: '10px',
                        textAlign: 'left',
                        borderRadius: '3px',
                        border: active
                          ? '1px solid var(--hsr-gold)'
                          : '1px solid var(--hsr-line)',
                        background: active
                          ? 'rgba(237, 208, 154, 0.1)'
                          : 'rgba(13, 17, 32, 0.65)',
                      }}
                    >
                      <HsrAssetImage
                        src={toHsrAssetUrl(aura.imagePath)}
                        alt={aura.name}
                        fallbackLabel={aura.name}
                        className="hsr-item-box hsr-item-box--md"
                      />
                      <div style={{ flex: 1 }}>
                        <div
                          style={{
                            display: 'flex',
                            justifyContent: 'space-between',
                            alignItems: 'center',
                            marginBottom: '4px',
                          }}
                        >
                          <strong style={{ color: 'var(--hsr-ink-warm)' }}>{aura.name}</strong>
                          {aura.difficultyIncrease > 0 && (
                            <HsrTag tone="crimson">敌人难度 +{aura.difficultyIncrease}</HsrTag>
                          )}
                        </div>
                        <p style={{ margin: 0, fontSize: '12px', color: '#d7d5e8' }}>
                          <HsrRichText text={aura.description} />
                        </p>
                      </div>
                    </button>
                  );
                })}
                <div style={{ display: 'flex', gap: '8px', marginTop: '4px' }}>
                  <HsrButton variant="secondary" size="sm" cost={1} style={{ flex: 1 }}>
                    刷新光环 (前6次1金)
                  </HsrButton>
                  <HsrButton variant="primary" size="sm" style={{ flex: 1 }}>
                    赋予光环
                  </HsrButton>
                </div>
              </div>
            </HsrPanel>

            <HsrPanel
              eyebrow="NODE SETTLEMENT"
              title={`节点 ${SETTLEMENT_FIXTURE.nodeId} 战斗结算`}
              accent="violet"
              rightSlot={<HsrTag tone="emerald">完胜 100%</HsrTag>}
            >
              <div className="hsr-stat-row">
                <span>小队血量变化</span>
                <strong>
                  {SETTLEMENT_FIXTURE.healthBefore} → {SETTLEMENT_FIXTURE.healthAfter} (+
                  {SETTLEMENT_FIXTURE.fixedRecovery})
                </strong>
              </div>
              <div className="hsr-stat-row">
                <span>基础战斗奖励</span>
                <strong>+{SETTLEMENT_FIXTURE.baseGold} 金币</strong>
              </div>
              <div className="hsr-stat-row">
                <span>金币利息结算</span>
                <strong>+{SETTLEMENT_FIXTURE.interestGold} 金币</strong>
              </div>
              <div className="hsr-stat-row">
                <span>火热连胜奖励 ({SETTLEMENT_FIXTURE.hotStreakAfter} 连胜)</span>
                <strong>+{SETTLEMENT_FIXTURE.streakGold} 金币</strong>
              </div>
              <div className="hsr-stat-row">
                <span>获得商店经验</span>
                <strong>+{SETTLEMENT_FIXTURE.experienceGained} EXP</strong>
              </div>
              <div style={{ marginTop: '12px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                {SETTLEMENT_FIXTURE.crystalsGranted.map((c) => (
                  <CrystalBadge
                    key={c.crystalId}
                    quality={c.quality}
                    label={c.label}
                    count={c.count}
                  />
                ))}
              </div>
            </HsrPanel>
          </div>
        )}
      </div>
    </div>
  );
}
