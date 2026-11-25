import { useEffect, useState } from 'react';
import { CharacterCard } from '../components/domain/CharacterCard';
import { InvestmentCard } from '../components/domain/InvestmentCard';
import { ItemFrameIcon } from '../components/domain/ItemFrameIcon';
import { CostBadge, HsrTag, PositionBadge } from '../components/primitives/HsrBadge';
import { HsrPanel } from '../components/primitives/HsrPanel';
import { HsrRichText } from '../components/primitives/HsrRichText';
import {
  type EncyclopediaCatalog,
  loadEncyclopediaCatalog,
} from '../data/encyclopediaLoader';
import type { ShowcaseCharacter } from '../types/hsr';

type ArchiveCategory = 'characters' | 'items' | 'strategies';

export function LiveDataArchiveView() {
  const [category, setCategory] = useState<ArchiveCategory>('characters');
  const [catalog, setCatalog] = useState<EncyclopediaCatalog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string>('');
  const [costFilter, setCostFilter] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;

    loadEncyclopediaCatalog()
      .then((loaded) => {
        if (!cancelled) {
          setCatalog(loaded);
          setSelectedId(loaded.characters[0]?.id ?? '');
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : '未知加载错误');
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (error) {
    return (
      <div style={{ padding: '48px 24px', maxWidth: '640px', margin: '0 auto' }}>
        <HsrPanel eyebrow="DATA BRIDGE" title="未能连接到 hsr-currency-wars-data" accent="gold">
          <p style={{ margin: '0 0 8px', color: 'var(--hsr-crimson)' }}>{error}</p>
          <p style={{ margin: 0, fontSize: '12px', color: 'var(--hsr-muted)' }}>
            请确认同级目录 <code>D:\HSR-Currency-War\hsr-currency-wars-data</code> 存在，或通过{' '}
            <code>HSR_CURRENCY_WARS_DATA</code> 环境变量指定路径。
          </p>
        </HsrPanel>
      </div>
    );
  }

  if (!catalog) {
    return (
      <div style={{ padding: '80px 24px', textAlign: 'center', color: 'var(--hsr-muted)' }}>
        正在从 hsr-currency-wars-data 载入真实图鉴数据...
      </div>
    );
  }

  const filteredChars =
    costFilter === null
      ? catalog.characters
      : catalog.characters.filter((c) => c.allCosts.includes(costFilter));

  const selectedChar = catalog.charactersById[selectedId] ?? filteredChars[0];
  const selectedItem = catalog.itemsById[selectedId] ?? catalog.items[0];
  const selectedStrategy = catalog.strategiesById[selectedId] ?? catalog.strategies[0];

  const switchCategory = (next: ArchiveCategory): void => {
    setCategory(next);
    if (next === 'characters') {
      setSelectedId(catalog.characters[0]?.id ?? '');
    } else if (next === 'items') {
      setSelectedId(catalog.items[0]?.id ?? '');
    } else {
      setSelectedId(catalog.strategies[0]?.id ?? '');
    }
  };

  return (
    <div className="hsr-archive-layout">
      <div className="hsr-archive-list">
        <div className="hsr-archive-toolbar">
          <div className="hsr-subtabs">
            <button
              type="button"
              className={`hsr-subtab ${category === 'characters' ? 'is-active' : ''}`}
              onClick={() => switchCategory('characters')}
            >
              角色图鉴 ({catalog.characters.length})
            </button>
            <button
              type="button"
              className={`hsr-subtab ${category === 'items' ? 'is-active' : ''}`}
              onClick={() => switchCategory('items')}
            >
              装备与改件 ({catalog.items.length})
            </button>
            <button
              type="button"
              className={`hsr-subtab ${category === 'strategies' ? 'is-active' : ''}`}
              onClick={() => switchCategory('strategies')}
            >
              投资策略 ({catalog.strategies.length})
            </button>
          </div>

          {category === 'characters' && (
            <div className="hsr-subtabs">
              <button
                type="button"
                className={`hsr-subtab ${costFilter === null ? 'is-active' : ''}`}
                onClick={() => setCostFilter(null)}
              >
                全部
              </button>
              {[1, 2, 3, 4, 5, 7].map((fee) => (
                <button
                  key={fee}
                  type="button"
                  className={`hsr-subtab ${costFilter === fee ? 'is-active' : ''}`}
                  onClick={() => setCostFilter(fee)}
                >
                  {fee}费
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="hsr-archive-grid">
          {category === 'characters' &&
            filteredChars.map((c) => {
              const cardChar: ShowcaseCharacter = {
                id: c.id,
                name: c.name,
                cost: c.cost,
                position: c.position,
                roles: c.roles,
                traits: c.traits,
                portraitPath: c.portraitPath,
                stars: 1,
                hp: 2400,
                speed: 120,
              };
              return (
                <CharacterCard
                  key={c.id}
                  character={cardChar}
                  mode="board"
                  selected={selectedChar?.id === c.id}
                  onClick={() => setSelectedId(c.id)}
                />
              );
            })}

          {category === 'items' &&
            catalog.items.map((item) => {
              const active = selectedItem?.id === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setSelectedId(item.id)}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: '7px',
                    padding: '10px 6px',
                    borderRadius: '3px',
                    background: active
                      ? 'rgba(237, 208, 154, 0.14)'
                      : 'rgba(18, 23, 42, 0.78)',
                    border: active
                      ? '1px solid var(--hsr-gold)'
                      : '1px solid var(--hsr-line)',
                  }}
                >
                  <ItemFrameIcon
                    imagePath={item.imagePath}
                    frame={item.frame}
                    name={item.name}
                    size="md"
                  />
                  <span style={{ fontSize: '12px', color: 'var(--hsr-ink)' }}>{item.name}</span>
                </button>
              );
            })}

          {category === 'strategies' &&
            catalog.strategies.slice(0, 60).map((s) => {
              const active = selectedStrategy?.id === s.id;
              return (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => setSelectedId(s.id)}
                  style={{
                    padding: '10px',
                    textAlign: 'left',
                    borderRadius: '3px',
                    background: active
                      ? 'rgba(237, 208, 154, 0.14)'
                      : 'rgba(18, 23, 42, 0.78)',
                    border: active
                      ? '1px solid var(--hsr-gold)'
                      : '1px solid var(--hsr-line)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '6px',
                  }}
                >
                  <HsrTag
                    tone={
                      s.quality === 'gold'
                        ? 'gold'
                        : s.quality === 'prismatic'
                          ? 'violet'
                          : 'default'
                    }
                  >
                    {s.qualityLabel}
                  </HsrTag>
                  <strong style={{ fontSize: '13px', color: 'var(--hsr-ink)' }}>{s.name}</strong>
                </button>
              );
            })}
        </div>
      </div>

      {/* Right Detail Inspector */}
      <HsrPanel
        eyebrow="ENCYCLOPEDIA INSPECTOR"
        title="条目详情"
        accent="gold"
        bodyClassName="hsr-archive-detail"
      >
        {category === 'characters' && selectedChar && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h3 style={{ margin: 0, fontSize: '20px' }}>{selectedChar.name}</h3>
              <CostBadge cost={selectedChar.allCosts.join(' / ')} />
            </div>
            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
              <PositionBadge position={selectedChar.position} />
              {selectedChar.roles.map((r) => (
                <HsrTag key={r} tone="gold">
                  {r}
                </HsrTag>
              ))}
              {selectedChar.traits.map((t) => (
                <HsrTag key={t} tone="violet">
                  {t}
                </HsrTag>
              ))}
            </div>
            <p style={{ margin: 0, fontSize: '13px', lineHeight: 1.8 }}>
              <HsrRichText text={selectedChar.description} />
            </p>
          </div>
        )}

        {category === 'items' && selectedItem && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <ItemFrameIcon
                imagePath={selectedItem.imagePath}
                frame={selectedItem.frame}
                name={selectedItem.name}
                size="lg"
              />
              <div>
                <h3 style={{ margin: '0 0 6px', fontSize: '18px' }}>{selectedItem.name}</h3>
                <HsrTag tone="gold">{selectedItem.type}</HsrTag>
              </div>
            </div>
            <p style={{ margin: 0, fontSize: '13px', lineHeight: 1.8 }}>
              <HsrRichText text={selectedItem.description} />
            </p>
          </div>
        )}

        {category === 'strategies' && selectedStrategy && (
          <div style={{ display: 'flex', justifyContent: 'center' }}>
            <InvestmentCard
              kind="strategy"
              title={selectedStrategy.name}
              quality={selectedStrategy.quality}
              qualityLabel={selectedStrategy.qualityLabel}
              description={selectedStrategy.description}
              imagePath={selectedStrategy.imagePath}
            />
          </div>
        )}
      </HsrPanel>
    </div>
  );
}
