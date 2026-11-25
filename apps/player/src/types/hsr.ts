export type Plane = 1 | 2 | 3;

declare module 'react' {
  interface CSSProperties {
    '--card-top'?: string;
    '--card-bottom'?: string;
  }
}
export type NodeId = `${Plane}-${number}`;
export type NodeKind = 'reward' | 'battle' | 'encounter' | 'boss' | 'supply';
export type CrystalQuality = 'white' | 'blue' | 'gold' | 'prismatic';
export type InvestmentQuality = 'silver' | 'gold' | 'prismatic';
export type CostTier = 1 | 2 | 3 | 4 | 5 | 7;
export type PositionKind = '前台' | '后台' | '前后台';

export type ItemFrameKind =
  | 'basic'
  | 'normal'
  | 'privilege'
  | 'spotlight'
  | 'aha'
  | 'food'
  | 'trailblaze'
  | 'hack-1'
  | 'hack-2'
  | 'hack-3';

export type TraitTier = 'inactive' | 'bronze' | 'silver' | 'gold' | 'prismatic';

export interface RouteNodeItem {
  nodeId: NodeId;
  plane: Plane;
  kind: NodeKind;
  status: 'done' | 'current' | 'upcoming';
}

export interface ShowcaseItem {
  id: string;
  name: string;
  type: string;
  frame: ItemFrameKind;
  imagePath: string;
  description: string;
  count?: number;
}

export interface ShowcaseCharacter {
  id: string;
  name: string;
  cost: CostTier;
  position: PositionKind;
  roles: string[];
  traits: string[];
  portraitPath: string;
  stars: 1 | 2 | 3 | 4;
  hp: number;
  speed: number;
  frontPower?: number;
  backPower?: number;
}

export interface BoardPiece {
  pieceId: string;
  zone: 'front' | 'back' | 'bench';
  slotIndex: number;
  character: ShowcaseCharacter;
  equipment: ShowcaseItem[];
  spotlightAura?: {
    id: string;
    name: string;
    imagePath: string;
  };
}

export interface ShowcaseTrait {
  id: string;
  name: string;
  group: '阵营' | '流派' | '独立';
  count: number;
  thresholds: number[];
  tier: TraitTier;
  imagePath: string;
  description: string;
  hasMechanism?: boolean;
  mechanismLabel?: string;
}

export interface ShopOfferCard {
  offerId: string;
  character: ShowcaseCharacter;
  price: CostTier;
  upgradeable?: boolean;
}

export interface CrystalEntry {
  crystalId: string;
  quality: CrystalQuality;
  label: string;
  count: number;
}

export interface ShowcaseStrategy {
  id: string;
  name: string;
  quality: InvestmentQuality;
  qualityLabel: string;
  description: string;
  imagePath: string;
  planes: Plane[];
  refreshAvailable?: boolean;
}

export interface ShowcaseEnvironment {
  id: string;
  name: string;
  description: string;
  imagePath: string;
  refreshAvailable?: boolean;
}

export interface ShowcaseAura {
  id: string;
  name: string;
  description: string;
  imagePath: string;
  difficultyIncrease: number;
}

export interface ShowcaseAffix {
  id: string;
  name: string;
  description: string;
}

export interface SupplyCombo {
  optionId: string;
  character: ShowcaseCharacter;
  items: ShowcaseItem[];
}

export interface SettlementReport {
  nodeId: NodeId;
  outcome: 'won' | 'lost';
  progressPercent: number;
  healthBefore: number;
  healthAfter: number;
  baseHealthLost: number;
  progressHealthLost: number;
  fixedRecovery: number;
  baseGold: number;
  interestGold: number;
  streakGold: number;
  hotStreakAfter: number;
  experienceGained: number;
  crystalsGranted: CrystalEntry[];
}

export interface AiRecommendation {
  id: string;
  title: string;
  tag: string;
  goldCost?: number;
  sssDelta: string;
  rationale: string;
}

/* Live Encyclopedia JSON Types (for LiveDataArchiveView) */
export interface EncyclopediaCharacter {
  id: string;
  name: string;
  description: string;
  cost: number[];
  roles: string[];
  traits: string[];
  position: string;
}

export interface EncyclopediaItem {
  id: string;
  name: string;
  description: string;
  type: string;
}

export interface EncyclopediaTrait {
  id: string;
  name: string;
  description: string;
}

export interface EncyclopediaStrategy {
  id: string;
  name: string;
  description: string;
  quality: '白银' | '黄金' | '彩虹';
  planes?: number[];
}

export interface EncyclopediaPresentation {
  characters: Record<string, { portrait: string | null }>;
  images: {
    items?: Record<string, string>;
    strategies?: Record<string, string>;
    traits?: Record<string, string>;
    environments?: Record<string, string>;
    auras?: Record<string, string>;
  };
  trait_groups: Record<string, string>;
}

export interface EncyclopediaItemDetail {
  frame?: string;
}
