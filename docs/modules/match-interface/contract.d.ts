/** Simulated Currency Wars match contract. JSON fields use camelCase. */
export type Plane = 1 | 2 | 3;
export type NodeId = `${Plane}-${number}`;
export type NodeKind = "reward" | "battle" | "encounter" | "boss" | "supply";
export type CrystalQuality = "white" | "blue" | "gold" | "prismatic";
export type InvestmentQuality = "silver" | "gold" | "prismatic";
export type Controller = { kind: "manual" } | { kind: "model"; modelId: string };

export interface CreateMatchInput {
  accountId: string;
  rulesVersion: string;
  /** Finite number strictly below 300. Validated by the service. */
  enemyDifficulty: number;
  affixCount: 0 | 1 | 2 | 3 | 4;
  randomness: { kind: "automatic" } | { kind: "seeded"; seed: string };
}

export interface MatchRef { matchId: string }
export interface MatchVersion extends MatchRef { revision: number }
export interface ExecuteActionInput extends MatchVersion { actionId: string }
export interface SetControllerInput extends MatchVersion { controller: Controller }

export interface RuleEntry { id: string; name: string; detail?: string }
export interface RouteNode { nodeId: NodeId; kind: NodeKind }
export interface RoutePosition extends RouteNode {
  index: number;
  enteredTriggerNodes: number;
}
export interface OpponentPreview {
  plane: Plane;
  enemies: RuleEntry[];
  bosses: RuleEntry[];
}

export interface Slot { zone: "front" | "back" | "bench"; index: number }
export interface Piece {
  pieceId: string;
  unitId: string;
  stars: number;
  slot: Slot;
  equipment: ItemStack[];
}
export interface TraitProgress {
  traitId: string;
  count: number;
  activeThreshold: number | null;
  thresholds: number[];
}
export interface ItemStack { itemId: string; definitionId: string; count: number }
export interface CrystalStack { crystalId: string; quality: CrystalQuality; count: number }
export interface ShopOffer { offerId: string; unitId: string; price: number }
export interface Resources {
  health: number;
  maxHealth: number;
  gold: number;
  hotStreak: number;
  interestCap: number;
  level: number;
  experience: number;
  experienceToNextLevel: number | null;
}

export interface Preparation {
  resources: Resources;
  shop: { slots: (ShopOffer | null)[]; locked: boolean; freeRefreshes: number };
  board: { front: number; back: number; bench: number; active: number; pieces: Piece[] };
  inventory: ItemStack[];
  crystals: CrystalStack[];
  traits: TraitProgress[];
  selectedInvestments: RuleEntry[];
  selectedAuras: (RuleEntry & { pieceId: string; effectActive: boolean; difficultyIncrease: number })[];
  revealedEnemies: RuleEntry[];
  mechanisms: VisibleMechanism[];
}

export interface VisibleMechanism {
  kind: "aha" | "wishPower" | "trailblazeShop" | "chimeraGarden";
  counters: { id: string; value: number }[];
  choices: TraitChoice[];
}

export interface TraitChoice {
  choiceId: string;
  mode: "once" | "changeable";
  selectedOptionId: string | null;
  options: ChoiceOption[];
}

export interface OpenTraitChoice {
  mechanism: VisibleMechanism["kind"];
  choiceId: string;
  draftOptionId: string | null;
}

export interface ChoiceOption extends RuleEntry { optionId: string }
export interface RefreshableChoice extends ChoiceOption { refreshAvailable: boolean }
export interface SupplyOption {
  optionId: string;
  unitId: string;
  itemDefinitionIds: string[];
}

export interface RevisitableSelection {
  selectionId: string;
  view: "selection" | "preparation";
}

export type MatchPhase =
  | { kind: "investmentEnvironment"; selectionId: string; options: RefreshableChoice[] }
  | { kind: "preparation"; spotlightAvailable: boolean; openTraitChoice: OpenTraitChoice | null }
  | (RevisitableSelection & { kind: "investmentStrategy"; options: (RefreshableChoice & { quality: InvestmentQuality })[] })
  | (RevisitableSelection & { kind: "supply"; options: SupplyOption[]; refreshAvailable: boolean })
  | (RevisitableSelection & { kind: "spotlight"; options: ChoiceOption[]; eligiblePieceIds: string[]; refreshCount: number; discountedRefreshLimit: number; refreshPrice: number })
  | { kind: "teaBreak"; selectionId: string; options: ChoiceOption[] }
  | { kind: "teaBreakReward"; selectionId: string; selectedOptionId: string }
  | { kind: "battle"; attemptId: string }
  | { kind: "battleUnavailable"; attemptId: string; message: string }
  | { kind: "battleAway" }
  | { kind: "finished"; reason: "cleared" | "healthDepleted" };

type ChoiceKind = "investmentEnvironment" | "investmentStrategy" | "spotlight";
type ItemTarget = { kind: "piece"; pieceId: string } | { kind: "item"; itemId: string };
export type MatchAction =
  | { kind: "returnToPreparation" | "returnToSelection"; selectionId: string }
  | { kind: "refreshChoice"; choice: ChoiceKind; selectionId: string; optionId: string }
  | { kind: "chooseInvestmentEnvironment" | "chooseInvestmentStrategy"; selectionId: string; optionId: string }
  | { kind: "refreshSupply"; selectionId: string }
  | { kind: "takeSupply"; selectionId: string; optionId: string }
  | { kind: "openSpotlight" }
  | { kind: "chooseSpotlight"; selectionId: string; optionId: string; pieceId: string }
  | { kind: "chooseTeaBreak"; selectionId: string; optionId: string }
  | { kind: "claimTeaBreakReward"; selectionId: string }
  | { kind: "buy"; offerId: string }
  | { kind: "sell"; pieceId: string }
  | { kind: "move"; pieceId: string; destination: Slot }
  | { kind: "refreshShop" | "buyExperience" }
  | { kind: "lockShop"; locked: boolean }
  | { kind: "openCrystal"; crystalId: string }
  | { kind: "equip"; itemId: string; pieceId: string }
  | { kind: "combine"; itemId: string; otherItemId: string }
  | { kind: "useItem"; itemId: string; target: ItemTarget }
  | { kind: "chooseMechanism"; mechanism: VisibleMechanism["kind"]; choiceId: string; optionId: string }
  | { kind: "openTraitChoice" | "confirmTraitChoice"; mechanism: VisibleMechanism["kind"]; choiceId: string }
  | { kind: "selectTraitOption"; mechanism: VisibleMechanism["kind"]; choiceId: string; optionId: string }
  | { kind: "startBattle" | "restartBattle" | "leaveBattle" | "reenterBattle" };

export interface LegalAction {
  actionId: string;
  action: MatchAction;
  goldCost?: number;
  source?: { kind: "trait"; traitId: string; mechanism?: VisibleMechanism["kind"] };
}

export interface NodeSettlement {
  nodeId: NodeId;
  source: "battle" | "teaBreak";
  healthBefore: number;
  healthAfter: number;
  healthLost: number;
  fixedRecovery: number;
  otherHealthDelta: number;
  baseGold: number;
  interestGold: number;
  streakGold: number;
  hotStreakAfter: number;
  experienceGained: number;
  crystalsGranted: CrystalStack[];
}

export interface MatchSnapshot extends MatchVersion {
  rulesVersion: string;
  accountId: string;
  accountConfigVersion: string;
  controller: Controller;
  route: RouteNode[];
  position: RoutePosition | null;
  phase: MatchPhase;
  rival: RuleEntry | null;
  opponents: OpponentPreview[];
  /** Slot order. The first slot is an enhancement or its corresponding 贪饕 variant. */
  affixes: RuleEntry[];
  environment: RuleEntry | null;
  companion: RuleEntry | null;
  preparation: Preparation | null;
  legalActions: LegalAction[];
  latestSettlement: NodeSettlement | null;
}

export type MatchError =
  | { code: "matchNotFound" | "invalidInput" | "rulesUnavailable" | "forbidden"; message: string }
  | { code: "revisionConflict"; currentRevision: number }
  | { code: "controllerMismatch"; controller: Controller }
  | { code: "actionUnavailable"; phase: MatchPhase["kind"] };
export type MatchResult<T> = { ok: true; value: T } | { ok: false; error: MatchError };

export type MatchUpdate =
  | { kind: "snapshot"; snapshot: MatchSnapshot }
  | (MatchVersion & { kind: "actionApplied"; actor: Controller; action: MatchAction })
  | (MatchVersion & { kind: "controllerChanged"; controller: Controller })
  | (MatchVersion & { kind: "battleUnavailable"; attemptId: string; message: string })
  | (MatchVersion & { kind: "battleSettled"; attemptId: string; settlement: NodeSettlement });

export interface MatchService {
  createMatch(input: CreateMatchInput): Promise<MatchResult<MatchSnapshot>>;
  getMatch(input: MatchRef): Promise<MatchResult<MatchSnapshot>>;
  executeAction(input: ExecuteActionInput): Promise<MatchResult<MatchSnapshot>>;
  setController(input: SetControllerInput): Promise<MatchResult<MatchSnapshot>>;
  subscribeMatch(input: MatchRef): Promise<MatchResult<AsyncIterable<MatchUpdate>>>;
}

/** Internal Rust boundary. The battle module owns the assembled input type. */
export interface BattleStart<BattleInput> extends MatchRef {
  nodeId: NodeId;
  attemptId: string;
  battleSeed: string;
  input: BattleInput;
}
export type BattleProgress =
  | { kind: "complete" }
  | { kind: "incomplete"; percent: number };
export interface BattleCompletion extends MatchRef {
  attemptId: string;
  outcome: "won" | "lost";
  progress: BattleProgress;
  crystalDrops: { quality: CrystalQuality; count: number }[];
}
export interface MatchBattlePort<BattleInput> {
  start(input: BattleStart<BattleInput>): void;
  cancel(input: MatchRef & { attemptId: string }): void;
}
export interface MatchBattleResultSink {
  complete(input: BattleCompletion): Promise<"applied" | "inactiveAttempt">;
  reportFailure(input: MatchRef & { attemptId: string; message: string }): Promise<"applied" | "inactiveAttempt">;
}
