import { itemFrameSvgUrl } from '../../assets/paths';
import type { ItemFrameKind } from '../../types/hsr';
import { HsrAssetImage } from '../primitives/HsrAssetImage';

export interface ItemFrameIconProps {
  imagePath: string | null | undefined;
  frame?: ItemFrameKind;
  name: string;
  size?: 'xs' | 'sm' | 'md' | 'lg';
  count?: number;
  className?: string;
}

export function ItemFrameIcon({
  imagePath,
  frame = 'normal',
  name,
  size = 'md',
  count,
  className,
}: ItemFrameIconProps) {
  return (
    <span
      className={`hsr-item-box hsr-item-box--${size} ${className ?? ''}`.trim()}
      title={name}
    >
      <HsrAssetImage
        src={imagePath}
        alt={name}
        fallbackLabel={name}
        className="hsr-item-art"
      />
      <img
        src={itemFrameSvgUrl(frame)}
        alt=""
        aria-hidden="true"
        className="hsr-item-frame"
      />
      {count !== undefined && count > 1 && (
        <span className="hsr-item-count">{count}</span>
      )}
    </span>
  );
}
