import type { CSSProperties } from 'react';
import {
  FEE_PALETTE,
  positionIconSvgUrl,
  starIconUrl,
} from '../../assets/paths';
import type { ShowcaseCharacter, ShowcaseItem } from '../../types/hsr';
import { HsrAssetImage } from '../primitives/HsrAssetImage';
import { CostBadge } from '../primitives/HsrBadge';
import { ItemFrameIcon } from './ItemFrameIcon';

export interface CharacterCardProps {
  character: ShowcaseCharacter;
  mode?: 'board' | 'bench' | 'shop' | 'gallery';
  equipment?: ShowcaseItem[];
  spotlightAuraName?: string;
  price?: number;
  upgradeable?: boolean;
  selected?: boolean;
  onClick?: () => void;
}

export function CharacterCard({
  character,
  mode = 'board',
  equipment = [],
  spotlightAuraName,
  price,
  upgradeable = false,
  selected = false,
  onClick,
}: CharacterCardProps) {
  const [topColor, bottomColor] = FEE_PALETTE[character.cost];
  const cardStyle: CSSProperties = {
    '--card-top': topColor,
    '--card-bottom': bottomColor,
  };

  return (
    <button
      type="button"
      style={cardStyle}
      onClick={onClick}
      className={`hsr-char-card hsr-char-card--${mode} ${selected ? 'is-selected' : ''}`.trim()}
      aria-pressed={selected}
      title={`${character.name} · ${character.cost}费 · ${character.position}`}
    >
      <HsrAssetImage
        src={character.portraitPath}
        alt={character.name}
        fallbackLabel={character.name}
        className="hsr-char-portrait"
      />

      <div className="hsr-char-top-bar">
        {mode !== 'shop' ? (
          <span className="hsr-char-star-pill">
            <img src={starIconUrl(character.stars)} alt={`${character.stars}星`} />
          </span>
        ) : (
          <CostBadge cost={price ?? character.cost} />
        )}

        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          {spotlightAuraName && (
            <span className="hsr-char-aura-pip" title={`主角光环：${spotlightAuraName}`}>
              ✦
            </span>
          )}
          <span className="hsr-char-pos-pill" title={character.position}>
            <img
              src={positionIconSvgUrl(character.position)}
              alt={character.position}
            />
          </span>
        </div>
      </div>

      {upgradeable && mode === 'shop' && (
        <span className="hsr-char-upgrade-pip">可升星</span>
      )}

      {equipment.length > 0 && mode !== 'bench' && (
        <div className="hsr-char-equip-row">
          {equipment.slice(0, 3).map((item) => (
            <ItemFrameIcon
              key={item.id}
              imagePath={item.imagePath}
              frame={item.frame}
              name={item.name}
              size="xs"
            />
          ))}
        </div>
      )}

      <div className="hsr-char-footer">
        <div style={{ minWidth: 0, flex: 1 }}>
          <div className="hsr-char-name">{character.name}</div>
          {mode !== 'bench' && (
            <div className="hsr-char-traits-mini">
              {character.traits.slice(0, 2).join(' · ')}
            </div>
          )}
        </div>
        {mode === 'gallery' && <CostBadge cost={character.cost} />}
      </div>
    </button>
  );
}
