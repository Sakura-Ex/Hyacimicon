/** The backend supplies display values and resolved artwork keys from the bundled catalogue. */
export interface Unit {
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
  title: string;
  level: number | null;
  description: string;
  scaling: Stat[];
}
export interface Character extends Unit {
  path: string;
  level: number | null;
  ascension: number | null;
  eidolons: number | null;
  portrait?: string;
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
  accounts(): Promise<Account[]>;
  importAccount(fileName: string, json: string): Promise<Account>;
  generateAccount(seed: number): Promise<Account>;
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
export interface BattlePort {
  battle(matchId: string): Promise<Frame[]>;
}
export type PlayerPort = AccountPort & MatchPort & BattlePort;
