import type { ShowcaseTrait } from '../../types/hsr';
import { HsrAssetImage } from '../primitives/HsrAssetImage';

export interface TraitBadgeProps {
  trait: ShowcaseTrait;
  active?: boolean;
  onClick?: () => void;
}

export function TraitBadge({ trait, active = false, onClick }: TraitBadgeProps) {
  const isInactive = trait.tier === 'inactive';

  return (
    <button
      type="button"
      onClick={onClick}
      className={`hsr-trait-badge ${isInactive ? 'hsr-trait-badge--inactive' : ''} ${active ? 'is-active' : ''}`.trim()}
      aria-pressed={active}
    >
      <span className={`hsr-trait-hex hsr-trait-hex--${trait.tier}`}>
        <span className="hsr-trait-hex-inner">
          <HsrAssetImage
            src={trait.imagePath}
            alt={trait.name}
            fallbackLabel={trait.name}
          />
        </span>
      </span>

      <div className="hsr-trait-info">
        <div className="hsr-trait-top">
          <span className="hsr-trait-name">{trait.name}</span>
          <span className="hsr-trait-count">{trait.count}</span>
        </div>

        <div className="hsr-trait-steps">
          {trait.thresholds.map((step, idx) => {
            const reached = trait.count >= step;
            return (
              <span
                key={step}
                className={`hsr-trait-step ${reached ? 'is-reached' : ''}`.trim()}
              >
                {idx > 0 ? ' / ' : ''}
                {step}
              </span>
            );
          })}
          {trait.hasMechanism && (
            <span className="hsr-trait-mech-dot">
              {trait.mechanismLabel ?? '机制'}
            </span>
          )}
        </div>
      </div>
    </button>
  );
}
