/** A real-game observation, recorded rather than inferred by a simulator. */
export const observationFields = [
  ["version", "游戏版本 / 赛季"], ["match", "对局标识"], ["node", "当前节点"], ["phase", "当前阶段"],
  ["health", "整局血量"], ["maxHealth", "血量上限"], ["gold", "金币"],
  ["level", "等级"], ["experience", "经验"], ["streak", "连胜"],
  ["freeRefreshes", "免费刷新次数"], ["shopLock", "商店锁定状态"],
  ["enemies", "已揭示敌人"], ["affixes", "负面词条"], ["environment", "投资环境"],
  ["investments", "投资策略"], ["companion", "奇遇伙伴"], ["traits", "羁绊及已激活效果"],
  ["auras", "主角光环"], ["crystals", "晶矿（品质、数量）"], ["items", "装备与消耗品"],
  ["choices", "当前候选及刷新状态"], ["settlement", "最近实际结算"], ["action", "实际操作 / 备注"],
] as const;
export type ObservationField = typeof observationFields[number][0];
export type Zone = "front" | "back" | "bench";
export interface Observation {
  fields: Record<ObservationField, string>;
  shop: string[];
  board: Record<Zone, string[]>;
}
export interface RecordedObservation {
  revision: number;
  recordedAt: string;
  observation: Observation;
}
export function emptyObservation(): Observation {
  return {
    fields: { version: "", match: "", node: "", phase: "", health: "", maxHealth: "", gold: "", level: "", experience: "", streak: "", freeRefreshes: "", shopLock: "", enemies: "", affixes: "", environment: "", investments: "", companion: "", traits: "", auras: "", crystals: "", items: "", choices: "", settlement: "", action: "" },
    shop: Array<string>(5).fill(""),
    board: { front: Array<string>(4).fill(""), back: Array<string>(9).fill(""), bench: Array<string>(9).fill("") },
  };
}
function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function strings(value: unknown, count: number): value is string[] {
  return Array.isArray(value) && value.length === count && value.every(entry => typeof entry === "string");
}
export function isObservation(value: unknown): value is Observation {
  if (!record(value) || !record(value.fields)) return false;
  const fields = value.fields;
  return observationFields.every(([key]) => typeof fields[key] === "string") &&
    strings(value.shop, 5) && record(value.board) && strings(value.board.front, 4) && strings(value.board.back, 9) && strings(value.board.bench, 9);
}
export function validateObservation(value: Observation): string | undefined {
  for (const key of ["health", "maxHealth", "gold", "level", "experience", "streak", "freeRefreshes"] as const) {
    const input = value.fields[key].trim();
    if (input && (!/^\d+$/.test(input) || !Number.isSafeInteger(Number(input)))) return `${observationFields.find(([id]) => id === key)?.[1]}应为非负整数或留空。`;
  }
  const { health, maxHealth } = value.fields;
  if (health.trim() && maxHealth.trim() && Number(health) > Number(maxHealth)) return "整局血量不能大于血量上限。";
  if (!value.fields.match.trim() || !value.fields.node.trim() || !value.fields.phase.trim()) return "请填写对局标识、当前节点和阶段。";
}
export function decodeHistory(raw: string | null): RecordedObservation[] {
  if (raw === null) return [];
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed) || !parsed.every((entry, i) => record(entry) && entry.revision === i + 1 &&
    typeof entry.recordedAt === "string" && isObservation(entry.observation) && !validateObservation(entry.observation))) {
    throw new Error("局面记录格式不正确。");
  }
  return parsed;
}
export function appendObservation(history: readonly RecordedObservation[], observation: Observation, recordedAt: string): RecordedObservation[] {
  const error = validateObservation(observation);
  if (error) throw new Error(error);
  return [...history, { revision: history.length + 1, recordedAt, observation: structuredClone(observation) }];
}
