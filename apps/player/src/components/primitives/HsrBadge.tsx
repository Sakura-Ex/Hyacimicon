import type { ReactNode } from 'react';
import { COIN_ICON_URL, positionIconSvgUrl } from '../../assets/paths';
import type { PositionKind } from '../../types/hsr';

export type HsrTagTone = 'default' | 'gold' | 'violet' | 'crimson' | 'emerald';

export function HsrTag({
  tone = 'default',
  children,
  className,
}: {
  tone?: HsrTagTone;
  children: ReactNode;
  className?: string;
}) {
  const toneClass = tone === 'default' ? '' : `hsr-tag--${tone}`;
  return <span className={`hsr-tag ${toneClass} ${className ?? ''}`.trim()}>{children}</span>;
}

export function CostBadge({ cost }: { cost: number | string }) {
  return (
    <span className="hsr-cost-badge">
      <img src={COIN_ICON_URL} alt="" aria-hidden="true" />
      <span>{cost}</span>
    </span>
  );
}

export function PositionBadge({
  position,
  showLabel = true,
}: {
  position: PositionKind;
  showLabel?: boolean;
}) {
  return (
    <span className="hsr-pos-badge" title={position}>
      <img src={positionIconSvgUrl(position)} alt={position} />
      {showLabel && <span>{position}</span>}
    </span>
  );
}
