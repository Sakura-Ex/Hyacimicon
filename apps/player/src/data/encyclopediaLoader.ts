import type {
  CostTier,
  InvestmentQuality,
  ItemFrameKind,
  PositionKind,
  ShowcaseAffix,
  ShowcaseAura,
  ShowcaseEnvironment,
} from '../types/hsr';
import { normalizeItemFrame } from '../assets/paths';

export interface CatalogCharacter {
  id: string;
  name: string;
  description: string;
  cost: CostTier;
  allCosts: number[];
  roles: string[];
  traits: string[];
  position: PositionKind;
  portraitPath: string;
}

export interface CatalogItem {
  id: string;
  name: string;
  description: string;
  type: string;
  frame: ItemFrameKind;
  imagePath: string;
}

export interface CatalogTraitTier {
  level: number;
  quality: string;
  text: string;
}

export interface CatalogTrait {
  id: string;
  name: string;
  description: string;
  group: '阵营' | '流派' | '独立';
  imagePath: string;
  tiers: CatalogTraitTier[];
}

export interface CatalogStrategy {
  id: string;
  name: string;
  description: string;
  quality: InvestmentQuality;
  qualityLabel: string;
  planes: number[];
  imagePath: string;
}

export interface CatalogRival {
  id: string;
  name: string;
  boss: string;
  eliteEnemies: string[];
  normalEnemies: string[];
}

export interface CatalogCompanion {
  id: string;
  name: string;
}

export interface EncyclopediaCatalog {
  characters: CatalogCharacter[];
  charactersById: Record<string, CatalogCharacter>;
  shopCharactersByCost: Record<number, CatalogCharacter[]>;
  items: CatalogItem[];
  itemsById: Record<string, CatalogItem>;
  basicItemIds: string[];
  advancedItemIds: string[];
  recipeLookup: Record<string, string>;
  traits: CatalogTrait[];
  traitsById: Record<string, CatalogTrait>;
  traitsByName: Record<string, CatalogTrait>;
  strategies: CatalogStrategy[];
  strategiesById: Record<string, CatalogStrategy>;
  strategiesByQuality: Record<InvestmentQuality, CatalogStrategy[]>;
  environments: ShowcaseEnvironment[];
  environmentsById: Record<string, ShowcaseEnvironment>;
  auras: ShowcaseAura[];
  aurasById: Record<string, ShowcaseAura>;
  affixes: ShowcaseAffix[];
  enhancementAffixes: ShowcaseAffix[];
  regularAffixes: ShowcaseAffix[];
  rivals: CatalogRival[];
  companions: CatalogCompanion[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function asNumber(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === 'string');
}

function asNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is number => typeof item === 'number' && Number.isFinite(item));
}

function toCostTier(raw: number | undefined): CostTier {
  if (raw === 1 || raw === 2 || raw === 3 || raw === 4 || raw === 5 || raw === 7) {
    return raw;
  }
  return 3;
}

function toPositionKind(raw: string): PositionKind {
  if (raw === '前台' || raw === '后台' || raw === '前后台') {
    return raw;
  }
  return '前台';
}

function toTraitGroup(raw: string): '阵营' | '流派' | '独立' {
  if (raw === '阵营' || raw === '流派') {
    return raw;
  }
  return '独立';
}

function toInvestmentQuality(raw: string): { quality: InvestmentQuality; label: string } {
  if (raw === '黄金') {
    return { quality: 'gold', label: '黄金' };
  }
  if (raw === '彩虹') {
    return { quality: 'prismatic', label: '棱彩' };
  }
  return { quality: 'silver', label: '白银' };
}

function getStringMap(root: Record<string, unknown>, key: string): Record<string, string> {
  const raw = root[key];
  if (!isRecord(raw)) {
    return {};
  }
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(raw)) {
    if (typeof v === 'string') {
      out[k] = v;
    }
  }
  return out;
}

export function buildCatalogFromRaw(raw: {
  characters: unknown;
  items: unknown;
  itemDetails: unknown;
  traits: unknown;
  traitDetails: unknown;
  strategies: unknown;
  environments: unknown;
  auras: unknown;
  affixes: unknown;
  rivals: unknown;
  companions: unknown;
  presentation: unknown;
}): EncyclopediaCatalog {
  if (!isRecord(raw.presentation)) {
    throw new Error('presentation.json 必须是对象');
  }
  const presCharactersRaw = isRecord(raw.presentation.characters) ? raw.presentation.characters : {};
  const presImagesRaw = isRecord(raw.presentation.images) ? raw.presentation.images : {};
  const itemImages = getStringMap(presImagesRaw, 'items');
  const traitImages = getStringMap(presImagesRaw, 'traits');
  const strategyImages = getStringMap(presImagesRaw, 'strategies');
  const envImages = getStringMap(presImagesRaw, 'environments');
  const auraImages = getStringMap(presImagesRaw, 'auras');
  const traitGroups = getStringMap(raw.presentation, 'trait_groups');

  const characters: CatalogCharacter[] = [];
  const charactersById: Record<string, CatalogCharacter> = {};
  const shopCharactersByCost: Record<number, CatalogCharacter[]> = {
    1: [],
    2: [],
    3: [],
    4: [],
    5: [],
  };

  if (Array.isArray(raw.characters)) {
    for (const entry of raw.characters) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const costs = asNumberArray(entry.cost);
      const primaryCost = toCostTier(costs[0]);
      const presEntry = presCharactersRaw[id];
      const portraitPath = isRecord(presEntry) ? asString(presEntry.portrait) : '';
      const char: CatalogCharacter = {
        id,
        name,
        description: asString(entry.description),
        cost: primaryCost,
        allCosts: costs.length > 0 ? costs : [primaryCost],
        roles: asStringArray(entry.roles),
        traits: asStringArray(entry.traits),
        position: toPositionKind(asString(entry.position)),
        portraitPath,
      };
      characters.push(char);
      charactersById[id] = char;
      if (primaryCost >= 1 && primaryCost <= 5) {
        shopCharactersByCost[primaryCost]?.push(char);
      }
    }
  }

  const itemDetailsRoot = isRecord(raw.itemDetails) ? raw.itemDetails : {};
  const items: CatalogItem[] = [];
  const itemsById: Record<string, CatalogItem> = {};
  const basicItemIds: string[] = [];
  const advancedItemIds: string[] = [];
  const recipeLookup: Record<string, string> = {};

  if (Array.isArray(raw.items)) {
    for (const entry of raw.items) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const detail = itemDetailsRoot[id];
      const rawFrame = isRecord(detail) ? asString(detail.frame) : undefined;
      const type = asString(entry.type);
      const item: CatalogItem = {
        id,
        name,
        description: asString(entry.description),
        type,
        frame: normalizeItemFrame(rawFrame),
        imagePath: itemImages[id] ?? '',
      };
      items.push(item);
      itemsById[id] = item;
      if (type === '简易装备') {
        basicItemIds.push(id);
      } else if (type === '进阶装备') {
        advancedItemIds.push(id);
      }

      if (isRecord(detail) && Array.isArray(detail.materials)) {
        const expanded: string[] = [];
        for (const mat of detail.materials) {
          if (!isRecord(mat)) {
            continue;
          }
          const matId = asString(mat.id);
          const cnt = asNumber(mat.count, 1);
          for (let c = 0; c < cnt; c += 1) {
            if (matId) {
              expanded.push(matId);
            }
          }
        }
        if (expanded.length === 2 && expanded[0] && expanded[1]) {
          const pair = [expanded[0], expanded[1]].sort();
          recipeLookup[`${pair[0]}+${pair[1]}`] = id;
        }
      }
    }
  }

  const traitDetailsObj =
    isRecord(raw.traitDetails) && isRecord(raw.traitDetails.traits)
      ? raw.traitDetails.traits
      : {};
  const traits: CatalogTrait[] = [];
  const traitsById: Record<string, CatalogTrait> = {};
  const traitsByName: Record<string, CatalogTrait> = {};

  if (Array.isArray(raw.traits)) {
    for (const entry of raw.traits) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const detail = traitDetailsObj[id];
      const tiers: CatalogTraitTier[] = [];
      if (isRecord(detail) && Array.isArray(detail.tiers)) {
        for (const t of detail.tiers) {
          if (isRecord(t)) {
            tiers.push({
              level: asNumber(t.level, 2),
              quality: asString(t.quality, 'None'),
              text: asString(t.text),
            });
          }
        }
      }
      const trait: CatalogTrait = {
        id,
        name,
        description: asString(entry.description),
        group: toTraitGroup(traitGroups[id] ?? ''),
        imagePath: traitImages[id] ?? '',
        tiers,
      };
      traits.push(trait);
      traitsById[id] = trait;
      traitsByName[name] = trait;
    }
  }

  const strategies: CatalogStrategy[] = [];
  const strategiesById: Record<string, CatalogStrategy> = {};
  const strategiesByQuality: Record<InvestmentQuality, CatalogStrategy[]> = {
    silver: [],
    gold: [],
    prismatic: [],
  };

  if (Array.isArray(raw.strategies)) {
    for (const entry of raw.strategies) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const q = toInvestmentQuality(asString(entry.quality));
      const planes = asNumberArray(entry.planes);
      const strategy: CatalogStrategy = {
        id,
        name,
        description: asString(entry.description),
        quality: q.quality,
        qualityLabel: q.label,
        planes: planes.length > 0 ? planes : [1, 2, 3],
        imagePath: strategyImages[id] ?? '',
      };
      strategies.push(strategy);
      strategiesById[id] = strategy;
      strategiesByQuality[q.quality].push(strategy);
    }
  }

  const environments: ShowcaseEnvironment[] = [];
  const environmentsById: Record<string, ShowcaseEnvironment> = {};
  if (Array.isArray(raw.environments)) {
    for (const entry of raw.environments) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const env: ShowcaseEnvironment = {
        id,
        name,
        description: asString(entry.description),
        imagePath: envImages[id] ?? '',
      };
      environments.push(env);
      environmentsById[id] = env;
    }
  }

  const auras: ShowcaseAura[] = [];
  const aurasById: Record<string, ShowcaseAura> = {};
  if (Array.isArray(raw.auras)) {
    for (const entry of raw.auras) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const aura: ShowcaseAura = {
        id,
        name,
        description: asString(entry.description),
        imagePath: auraImages[id] ?? '',
        difficultyIncrease: asNumber(entry.enemy_difficulty_increase, 0),
      };
      auras.push(aura);
      aurasById[id] = aura;
    }
  }

  const affixes: ShowcaseAffix[] = [];
  const enhancementAffixes: ShowcaseAffix[] = [];
  const regularAffixes: ShowcaseAffix[] = [];
  if (Array.isArray(raw.affixes)) {
    for (const entry of raw.affixes) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      const affix: ShowcaseAffix = {
        id,
        name,
        description: asString(entry.description),
      };
      affixes.push(affix);
      if (name.includes('强化')) {
        enhancementAffixes.push(affix);
      } else {
        regularAffixes.push(affix);
      }
    }
  }

  const rivals: CatalogRival[] = [];
  if (Array.isArray(raw.rivals)) {
    for (const entry of raw.rivals) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (!id || !name) {
        continue;
      }
      rivals.push({
        id,
        name,
        boss: asString(entry.boss),
        eliteEnemies: asStringArray(entry.elite_enemies),
        normalEnemies: asStringArray(entry.normal_enemies),
      });
    }
  }

  const companions: CatalogCompanion[] = [];
  if (Array.isArray(raw.companions)) {
    for (const entry of raw.companions) {
      if (!isRecord(entry)) {
        continue;
      }
      const id = asString(entry.id);
      const name = asString(entry.name);
      if (id && name) {
        companions.push({ id, name });
      }
    }
  }

  return {
    characters,
    charactersById,
    shopCharactersByCost,
    items,
    itemsById,
    basicItemIds,
    advancedItemIds,
    recipeLookup,
    traits,
    traitsById,
    traitsByName,
    strategies,
    strategiesById,
    strategiesByQuality,
    environments,
    environmentsById,
    auras,
    aurasById,
    affixes,
    enhancementAffixes,
    regularAffixes,
    rivals,
    companions,
  };
}

async function fetchJson(url: string): Promise<unknown> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`加载失败 (${response.status}): ${url}`);
  }
  return response.json();
}

export async function loadEncyclopediaCatalog(): Promise<EncyclopediaCatalog> {
  const [
    characters,
    items,
    itemDetails,
    traits,
    traitDetails,
    strategies,
    environments,
    auras,
    affixes,
    rivals,
    companions,
    presentation,
  ] = await Promise.all([
    fetchJson('/hsr-data/characters.json'),
    fetchJson('/hsr-data/items.json'),
    fetchJson('/hsr-data/item_details.json'),
    fetchJson('/hsr-data/traits.json'),
    fetchJson('/hsr-data/trait_details.json'),
    fetchJson('/hsr-data/strategies.json'),
    fetchJson('/hsr-data/environments.json'),
    fetchJson('/hsr-data/auras.json'),
    fetchJson('/hsr-data/affixes.json'),
    fetchJson('/hsr-data/rivals.json'),
    fetchJson('/hsr-data/companions.json'),
    fetchJson('/hsr-data/presentation.json'),
  ]);

  return buildCatalogFromRaw({
    characters,
    items,
    itemDetails,
    traits,
    traitDetails,
    strategies,
    environments,
    auras,
    affixes,
    rivals,
    companions,
    presentation,
  });
}
