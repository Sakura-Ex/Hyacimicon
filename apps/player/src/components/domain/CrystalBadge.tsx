import type { CrystalQuality } from '../../types/hsr';

export interface CrystalBadgeProps {
  quality: CrystalQuality;
  label: string;
  count: number;
  onClick?: () => void;
}

export function CrystalBadge({ quality, label, count, onClick }: CrystalBadgeProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="hsr-crystal-badge"
      title={`开启 ${label}`}
    >
      <span className={`hsr-crystal-gem hsr-crystal-gem--${quality}`} aria-hidden="true" />
      <span className="hsr-crystal-label">{label}</span>
      <span className="hsr-crystal-count">×{count}</span>
    </button>
  );
}
