import { FEE_PALETTE } from '../assets/paths';
import { CharacterCard } from '../components/domain/CharacterCard';
import { CrystalBadge } from '../components/domain/CrystalBadge';
import { InvestmentCard } from '../components/domain/InvestmentCard';
import { ItemFrameIcon } from '../components/domain/ItemFrameIcon';
import { TraitBadge } from '../components/domain/TraitBadge';
import { CostBadge, HsrTag, PositionBadge } from '../components/primitives/HsrBadge';
import { HsrButton } from '../components/primitives/HsrButton';
import { HsrPanel } from '../components/primitives/HsrPanel';
import { HsrRichText } from '../components/primitives/HsrRichText';
import {
  CRYSTALS_FIXTURE,
  ENVIRONMENTS_FIXTURE,
  SHOWCASE_CHARACTERS,
  SHOWCASE_ITEMS,
  STRATEGIES_FIXTURE,
  TRAITS_FIXTURE,
} from '../data/showcaseFixtures';
import type { CostTier } from '../types/hsr';

const CORE_SWATCHES = [
  { name: '星穹深空 Void', token: '--hsr-void', value: '#0b0e1a' },
  { name: '黑曜玻璃 Glass', token: '--hsr-surface', value: '#1a1f38' },
  { name: '香槟金 Gold', token: '--hsr-gold', value: '#edd09a' },
  { name: '星轨紫 Violet', token: '--hsr-violet', value: '#c2b1ff' },
  { name: '冰晶青 Cyan', token: '--hsr-cyan', value: '#8ce5e5' },
  { name: '警示绯 Crimson', token: '--hsr-crimson', value: '#ef7c8e' },
];

const FEE_TIERS: CostTier[] = [1, 2, 3, 4, 5, 7];

const ALL_FRAME_ITEMS = [
  SHOWCASE_ITEMS.bootsBasic,
  SHOWCASE_ITEMS.antiGrav,
  SHOWCASE_ITEMS.privilegeBoots,
  SHOWCASE_ITEMS.spotlightBlade,
  SHOWCASE_ITEMS.foodLion,
  SHOWCASE_ITEMS.trailblazeMod,
  SHOWCASE_ITEMS.hackLv1,
  SHOWCASE_ITEMS.hackLv2,
  SHOWCASE_ITEMS.hackLv3,
];

const FEE_SAMPLES = [
  SHOWCASE_CHARACTERS.welt,
  SHOWCASE_CHARACTERS.yunli,
  SHOWCASE_CHARACTERS.feixiao,
  SHOWCASE_CHARACTERS.acheron,
  SHOWCASE_CHARACTERS.aglaea,
  SHOWCASE_CHARACTERS.aha,
];

export function DesignSystemView() {
  return (
    <div className="hsr-ds-page">
      <section>
        <h2 className="hsr-ds-section-title">01 · 色彩与官方费用阶梯令牌</h2>
        <div className="hsr-swatch-grid">
          {CORE_SWATCHES.map((s) => (
            <div key={s.token} className="hsr-swatch">
              <div className="hsr-swatch-color" style={{ background: s.value }} />
              <div className="hsr-swatch-meta">
                <strong>{s.name}</strong>
                <code>{s.value}</code>
              </div>
            </div>
          ))}
          {FEE_TIERS.map((tier) => {
            const [top, bottom] = FEE_PALETTE[tier];
            return (
              <div key={tier} className="hsr-swatch">
                <div
                  className="hsr-swatch-color"
                  style={{ background: `linear-gradient(155deg, ${top}, ${bottom})` }}
                />
                <div className="hsr-swatch-meta">
                  <strong>{tier} 费角色梯度</strong>
                  <code>
                    {top} → {bottom}
                  </code>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section>
        <h2 className="hsr-ds-section-title">02 · 基础交互控件与富文本规范</h2>
        <HsrPanel eyebrow="INTERACTIVE PRIMITIVES" title="按钮、标签与词条高亮" accent="gold">
          <div className="hsr-kit-row" style={{ marginBottom: '16px' }}>
            <HsrButton variant="primary" size="lg">
              开始战斗 (Primary CTA)
            </HsrButton>
            <HsrButton variant="secondary" size="md" cost={4}>
              购买经验
            </HsrButton>
            <HsrButton variant="ghost" size="md">
              返回备战
            </HsrButton>
            <HsrButton variant="danger" size="md">
              主动暂离
            </HsrButton>
            <HsrButton variant="circle-close" aria-label="关闭">
              ✕
            </HsrButton>
          </div>

          <div className="hsr-kit-row" style={{ marginBottom: '16px' }}>
            <HsrTag>常规标签</HsrTag>
            <HsrTag tone="gold">黄金状态</HsrTag>
            <HsrTag tone="violet">紫晶词条</HsrTag>
            <HsrTag tone="crimson">敌人强化</HsrTag>
            <HsrTag tone="emerald">已达标</HsrTag>
            <CostBadge cost={5} />
            <PositionBadge position="前台" />
            <PositionBadge position="后台" />
            <PositionBadge position="前后台" />
          </div>

          <p style={{ margin: 0, fontSize: '14px', lineHeight: 1.8 }}>
            <HsrRichText text="【至高之姿】状态下，阿格莱雅和队友行动会为其累积【剑锋】，在连携攻击后造成 25% 额外真实伤害，基础【连击率】提高 15.5%。" />
          </p>
        </HsrPanel>
      </section>

      <section>
        <h2 className="hsr-ds-section-title">03 · 1–7 费角色卡与星级矩阵</h2>
        <div className="hsr-kit-row">
          {FEE_SAMPLES.map((char) => (
            <CharacterCard
              key={char.id}
              character={char}
              mode="gallery"
              equipment={char.id === 'characters-003' ? [SHOWCASE_ITEMS.antiGrav] : []}
            />
          ))}
        </div>
      </section>

      <section>
        <h2 className="hsr-ds-section-title">04 · 全 9 种装备边框与晶矿体系</h2>
        <HsrPanel eyebrow="EQUIPMENT FRAMES" title="SVG 分层边框 + 晶矿品质">
          <div className="hsr-kit-row" style={{ marginBottom: '16px' }}>
            {ALL_FRAME_ITEMS.map((item) => (
              <div
                key={item.id}
                style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '6px' }}
              >
                <ItemFrameIcon
                  imagePath={item.imagePath}
                  frame={item.frame}
                  name={item.name}
                  size="lg"
                />
                <span style={{ fontSize: '11px', color: 'var(--hsr-muted)' }}>{item.frame}</span>
              </div>
            ))}
          </div>

          <div className="hsr-kit-row">
            {CRYSTALS_FIXTURE.map((c) => (
              <CrystalBadge key={c.crystalId} quality={c.quality} label={c.label} count={c.count} />
            ))}
          </div>
        </HsrPanel>
      </section>

      <section>
        <h2 className="hsr-ds-section-title">05 · 羁绊阶梯与投资卡片质感</h2>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: '10px',
            marginBottom: '18px',
          }}
        >
          {TRAITS_FIXTURE.slice(0, 5).map((trait) => (
            <TraitBadge key={trait.id} trait={trait} />
          ))}
        </div>

        <div className="hsr-cards-row" style={{ justifyContent: 'flex-start' }}>
          {STRATEGIES_FIXTURE.map((s) => (
            <InvestmentCard
              key={s.id}
              kind="strategy"
              title={s.name}
              quality={s.quality}
              qualityLabel={s.qualityLabel}
              description={s.description}
              imagePath={s.imagePath}
            />
          ))}
          {ENVIRONMENTS_FIXTURE[0] && (
            <InvestmentCard
              kind="environment"
              title={ENVIRONMENTS_FIXTURE[0].name}
              description={ENVIRONMENTS_FIXTURE[0].description}
              imagePath={ENVIRONMENTS_FIXTURE[0].imagePath}
            />
          )}
        </div>
      </section>
    </div>
  );
}
