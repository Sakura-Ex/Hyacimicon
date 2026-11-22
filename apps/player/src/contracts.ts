/** The backend supplies display values and resolved artwork keys from the bundled catalogue. */
export interface Unit {
  portrait?: string;
  id: string;
  name: string;
  image: string;
}
export interface Item {
  name: string;
  image: string;
  detail: string;
  count: number;
}
export type Stat = [label: string, value: string];
export interface Skill {
  group: string;
  tab: string;
  title: string | null;
  level: number | null;
  description: string;
  scaling: Stat[];
}
export interface Character extends Unit {
  path: string;
  level: number | null;
  ascension: number | null;
  eidolons: number | null;
  stats: Stat[];
  skills: Skill[];
  cone?: Item;
  relics: Item[];
}
export interface Account {
  id: string;
  name: string;
  source: string;
  characters: Character[];
  summary?: AccountSummary;
  origin?: AccountOrigin;
}
export interface AccountSummary {
  characters: number;
  lightCones: number;
  relics: number;
  idleLightCones: number;
  idleRelics: number;
}
export interface ImportReport {
  source: string;
  build: string;
  format_version: number;
  trace_bindings_snapshot: string;
  warnings: { path: string; message: string }[];
}
export interface ImportInspection {
  name: string;
  summary: AccountSummary;
  report: ImportReport | null;
}
export interface GenerationReport {
  goldCount: number;
  meanEffectiveRolls: number;
  coreCharacterIds: string[];
  coreGoldShare: number;
  coreRelicShare: number;
  attempts: number;
}
export type AccountOrigin =
  | { kind: "imported"; filename: string; report: ImportReport | null }
  | { kind: "generated"; seed: string; request: Record<string, unknown>; report: GenerationReport; generatorVersion: string };
export type AccountFileKind = "account" | "original" | "report" | "request";
export interface AccountLibrary {
  accounts: Account[];
  warnings: string[];
}
export interface AccountCatalogue {
  characters: { id: string; name: string }[];
  lightCones: { id: string; name: string }[];
}
export interface Piece extends Unit {
  stars: number | null;
  gear: Item[];
}
export interface ActionOption {
  id: string;
  title: string;
  expected: Record<string, number>;
}
export interface Advice extends ActionOption {
  reason: string;
  effects: Stat[];
  image?: string;
}
export interface Match {
  id: string;
  revision: number;
  node: string;
  health: number | null;
  streak: number | null;
  interest: number | null;
  capacity: number;
  gold: number | null;
  front: Piece[];
  back: Piece[];
  backCapacity: number;
  bench: Piece[];
  traits: Item[];
  inventory: Item[];
  consumables: Item[];
  offers: { unit: Unit; cost: number | null; note: string }[];
  actions: ActionOption[];
  advice: Advice[];
  feedbackFields: {
    key: string;
    label: string;
    current: number | null;
    min: number;
    max?: number;
  }[];
  refresh?: { actionId: string; label: string };
  lastAction?: string;
  locked: boolean;
}
export interface Feedback {
  matchId: string;
  revision: number;
  action: string;
  observed: Record<string, number>;
}
export interface Combatant extends Unit {
  side: string;
  element: string;
  hp: number | null;
  maxHp: number | null;
  energy: number | null;
  maxEnergy: number | null;
  stats: Stat[];
  gear: Item[];
  buffs: Item[];
  initiative: number | null;
}
export interface Frame {
  actorId: string;
  portrait?: string;
  skill: string;
  kind: string;
  targets: { unit: Unit; damage: number }[];
  units: Combatant[];
  log: Stat[];
}
export interface AccountPort {
  accounts(): Promise<AccountLibrary>;
  inspectAccount(fileName: string, json: string): Promise<ImportInspection>;
  importAccount(fileName: string, json: string, name: string): Promise<Account>;
  generateAccount(request: Record<string, unknown>, seed: string | null, name: string | null): Promise<Account>;
  accountCatalogue(): Promise<AccountCatalogue>;
  renameAccount(id: string, name: string): Promise<Account>;
  deleteAccount(id: string): Promise<void>;
  exportAccount(id: string, kind: AccountFileKind): Promise<string>;
}
export interface MatchPort {
  match(accountId: string): Promise<Match>;
  setShopLock(
    matchId: string,
    revision: number,
    locked: boolean,
  ): Promise<Match>;
  saveFeedback(input: Feedback): Promise<Match>;
}
export interface StandardBattle {
  mode: "standard";
  allyIds: string[];
  enemyIds: string[];
}
export type BattleInput =
  | StandardBattle
  | {
      mode: "currencyWars";
      matchId: string;
      revision: number;
    };
export interface BattleCatalogue {
  allies: Unit[];
  enemies: Unit[];
  initial: StandardBattle;
}
export type BattleResult =
  { kind: "preview"; units: Combatant[] } | { kind: "replay"; frames: Frame[] };
export interface BattlePort {
  battleCatalogue(): Promise<BattleCatalogue>;
  battle(input: BattleInput): Promise<BattleResult>;
}
export type PlayerPort = AccountPort & MatchPort & BattlePort;
