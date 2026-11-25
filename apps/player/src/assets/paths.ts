import type {
  CostTier,
  InvestmentQuality,
  ItemFrameKind,
  PositionKind,
} from '../types/hsr';

const ENCYCLOPEDIA_PREFIX = '/assets/encyclopedia/';

export const COIN_ICON_URL = '/hsr-assets/items/352608.webp';

export const FEE_PALETTE: Record<CostTier, readonly [string, string]> = {
  1: ['#606570', '#343842'],
  2: ['#406a56', '#293f36'],
  3: ['#3d6588', '#293d5b'],
  4: ['#70528c', '#3c3158'],
  5: ['#897351', '#433846'],
  7: ['#a78455', '#44334a'],
};

export function toHsrAssetUrl(rawPath: string | null | undefined): string | null {
  if (!rawPath) {
    return null;
  }
  if (rawPath.startsWith(ENCYCLOPEDIA_PREFIX)) {
    return `/hsr-assets/${rawPath.slice(ENCYCLOPEDIA_PREFIX.length)}`;
  }
  if (rawPath.startsWith('/hsr-assets/') || rawPath.startsWith('/hsr-svg/')) {
    return rawPath;
  }
  return rawPath;
}

export function itemFrameSvgUrl(frame: ItemFrameKind): string {
  return `/hsr-svg/frame-${frame}.svg`;
}

export function investmentBackplateSvgUrl(
  kind: 'environment' | 'strategy',
  quality: InvestmentQuality = 'silver',
): string {
  if (kind === 'environment') {
    return '/hsr-svg/backplate-environment.svg';
  }
  return `/hsr-svg/backplate-strategy-${quality}.svg`;
}

export function categoryIconSvgUrl(name: string): string {
  return `/hsr-svg/icons/${name}.svg`;
}

export function positionIconSvgUrl(pos: PositionKind): string {
  switch (pos) {
    case '前台':
      return '/hsr-svg/icons/frontstage.svg';
    case '后台':
      return '/hsr-svg/icons/backstage.svg';
    case '前后台':
      return '/hsr-svg/icons/dualstage.svg';
  }
}

export function starIconUrl(stars: 1 | 2 | 3 | 4): string {
  return `/hsr-assets/trait-stats/star-${stars}.png`;
}

export function resourceIconUrl(
  name:
    | 'experience'
    | 'free-refresh'
    | 'spotlight-change'
    | 'squad-health'
    | 'trailblaze-power',
): string {
  return `/hsr-assets/resources/${name}.png`;
}

export function normalizeItemFrame(rawFrame: string | undefined): ItemFrameKind {
  switch (rawFrame) {
    case 'basic':
    case 'normal':
    case 'privilege':
    case 'spotlight':
    case 'aha':
    case 'food':
    case 'trailblaze':
    case 'hack-1':
    case 'hack-2':
    case 'hack-3':
      return rawFrame;
    default:
      return 'normal';
  }
}
