import { useId } from 'react';
import { investmentBackplateSvgUrl, toHsrAssetUrl } from '../../assets/paths';
import type { InvestmentQuality } from '../../types/hsr';
import { HsrButton } from '../primitives/HsrButton';
import { HsrTag } from '../primitives/HsrBadge';
import { HsrRichText } from '../primitives/HsrRichText';

const QUALITY_GLYPH_STOPS: Record<InvestmentQuality, readonly (readonly [number, string])[]> = {
  silver: [
    [0, '#c9c9cf'],
    [0.28, '#d2d2d7'],
    [0.72, '#f0f0f3'],
    [1, '#f5f5f7'],
  ],
  gold: [
    [0, '#ffe49e'],
    [0.28, '#ffe9aa'],
    [0.72, '#fff3c4'],
    [1, '#fff6d1'],
  ],
  prismatic: [
    [0, '#f4b3fb'],
    [0.28, '#f4b3fb'],
    [0.48, '#b8fff6'],
    [0.72, '#ffffff'],
    [1, '#ffffff'],
  ],
};

export interface InvestmentCardProps {
  kind: 'strategy' | 'environment';
  title: string;
  description: string;
  imagePath: string;
  quality?: InvestmentQuality;
  qualityLabel?: string;
  selected?: boolean;
  refreshAvailable?: boolean;
  onSelect?: () => void;
  onRefresh?: () => void;
}

export function InvestmentCard({
  kind,
  title,
  description,
  imagePath,
  quality = 'silver',
  qualityLabel,
  selected = false,
  refreshAvailable = false,
  onSelect,
  onRefresh,
}: InvestmentCardProps) {
  const uid = useId().replace(/:/g, '');
  const resolvedArt = toHsrAssetUrl(imagePath);
  const backplateUrl = investmentBackplateSvgUrl(kind, quality);

  const width = 256;
  const height = kind === 'strategy' ? 280 : 336;
  const centerY = kind === 'strategy' ? 149 : 150;
  const glyphSize = width * (kind === 'strategy' ? 0.7 : 0.65);
  const x = (width - glyphSize) / 2;
  const y = centerY - glyphSize / 2;
  const stops = QUALITY_GLYPH_STOPS[quality];

  return (
    <article
      className={`hsr-inv-card ${kind === 'strategy' ? `hsr-inv-card--${quality}` : ''} ${selected ? 'is-selected' : ''}`.trim()}
    >
      <div
        className={`hsr-inv-art-wrap ${kind === 'environment' ? 'hsr-inv-art-wrap--environment' : `hsr-inv-art-wrap--${quality}`}`}
      >
        <img
          src={backplateUrl}
          alt=""
          aria-hidden="true"
          className="hsr-inv-backplate"
        />
        {resolvedArt && (
          <svg
            className="hsr-inv-svg"
            viewBox={`0 0 ${width} ${height}`}
            preserveAspectRatio={kind === 'environment' ? 'none' : 'xMidYMid meet'}
            aria-hidden="true"
          >
            {kind === 'strategy' ? (
              <>
                <defs>
                  <linearGradient
                    id={`${uid}-grad`}
                    gradientUnits="userSpaceOnUse"
                    x1="0"
                    y1={y}
                    x2="0"
                    y2={y + glyphSize}
                  >
                    {stops.map(([offset, color]) => (
                      <stop key={offset} offset={offset} stopColor={color} />
                    ))}
                  </linearGradient>
                  <mask
                    id={`${uid}-mask`}
                    maskUnits="userSpaceOnUse"
                    x="0"
                    y="0"
                    width={width}
                    height={height}
                    style={{ maskType: 'alpha' }}
                  >
                    <image
                      href={resolvedArt}
                      x={x}
                      y={y}
                      width={glyphSize}
                      height={glyphSize}
                    />
                  </mask>
                </defs>
                <rect
                  width={width}
                  height={height}
                  fill={`url(#${uid}-grad)`}
                  mask={`url(#${uid}-mask)`}
                />
              </>
            ) : (
              <image
                href={resolvedArt}
                x={x}
                y={y}
                width={glyphSize}
                height={glyphSize}
              />
            )}
          </svg>
        )}
      </div>

      {qualityLabel && (
        <HsrTag tone={quality === 'gold' ? 'gold' : quality === 'prismatic' ? 'violet' : 'default'}>
          {qualityLabel}
        </HsrTag>
      )}

      <h4 className="hsr-inv-title">{title}</h4>
      <p className="hsr-inv-desc">
        <HsrRichText text={description} />
      </p>

      {(onSelect || onRefresh) && (
        <div className="hsr-inv-actions">
          {onRefresh && (
            <HsrButton
              variant="ghost"
              size="sm"
              disabled={!refreshAvailable}
              onClick={onRefresh}
              style={{ flex: 1 }}
            >
              {refreshAvailable ? '刷新 1/1' : '已刷新'}
            </HsrButton>
          )}
          {onSelect && (
            <HsrButton
              variant={selected ? 'primary' : 'secondary'}
              size="sm"
              onClick={onSelect}
              style={{ flex: 1.4 }}
            >
              {selected ? '已选定' : '选择'}
            </HsrButton>
          )}
        </div>
      )}
    </article>
  );
}
