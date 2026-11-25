import { accountView } from "./account";
import { battleView } from "./battle";
import { battleWorkspace } from "./battle-workspace";
import { demoPort, demoRules } from "./demo";
import { demoSessionPort } from "./demo-match";
import { matchView, type MatchPreviewPanel } from "./match";
import { previewMatchBattle } from "./match-battle-preview";
import { matchSession } from "./match-session";
import type { BattleResult, Frame } from "./contracts";
import type { Run, Spread } from "./folio";

type PreviewTarget = { label: string } & (
  | { page: "account"; view: "archive" | "empty" | "import" | "generate" | "manage" }
  | { page: "match"; scenario?: string; panel?: MatchPreviewPanel; model?: boolean }
  | { page: "battle"; view: "standard" | "currencyWars" | "replay" }
);

export const previewGroups: { title: string; targets: PreviewTarget[] }[] = [
  { title: "账号", targets: [
    { page: "account", view: "archive", label: "角色档案" },
    { page: "account", view: "empty", label: "空账号" },
    { page: "account", view: "import", label: "文件导入" },
    { page: "account", view: "generate", label: "随机生成" },
    { page: "account", view: "manage", label: "账号管理" },
  ] },
  { title: "对局", targets: [
    { page: "match", label: "开局配置" },
    { page: "match", scenario: "investmentEnvironment", label: "投资环境" },
    { page: "match", scenario: "preparation", label: "常规备战" },
    { page: "match", scenario: "full", label: "满员阵容" },
    { page: "match", scenario: "stock", label: "长库存" },
    { page: "match", scenario: "planeOne", label: "第一位面进度" },
    { page: "match", scenario: "planeThree", label: "第三位面进度" },
    { page: "match", scenario: "preparation", panel: "piece", label: "角色详情" },
    { page: "match", scenario: "preparation", panel: "equipment", label: "装备详情" },
    { page: "match", scenario: "preparation", panel: "feedback", label: "实战回填" },
    { page: "match", scenario: "traitChoice", label: "羁绊选择" },
    { page: "match", scenario: "investmentEnvironment", model: true, label: "模型观战" },
    { page: "match", scenario: "investmentStrategy", label: "投资策略" },
    { page: "match", scenario: "investmentStrategyPreparation", label: "投资策略 · 返回备战" },
    { page: "match", scenario: "supply", label: "补给" },
    { page: "match", scenario: "supplyPreparation", label: "补给 · 返回备战" },
    { page: "match", scenario: "spotlight", label: "主角光环" },
    { page: "match", scenario: "spotlightPreparation", label: "主角光环 · 返回备战" },
    { page: "match", scenario: "teaBreak", label: "幕后茶歇" },
    { page: "match", scenario: "teaBreakReward", label: "茶歇领奖" },
    { page: "match", scenario: "battle", label: "战斗中" },
    { page: "match", scenario: "battleAway", label: "战斗暂离" },
    { page: "match", scenario: "battleUnavailable", label: "战斗暂不可用" },
    { page: "match", scenario: "finished", label: "终局 · 通关" },
    { page: "match", scenario: "healthDepleted", label: "终局 · 血量耗尽" },
  ] },
  { title: "战斗", targets: [
    { page: "battle", view: "standard", label: "常规战斗" },
    { page: "battle", view: "currencyWars", label: "货币战争阵容" },
    { page: "battle", view: "replay", label: "行动回放" },
  ] },
];

export interface DeveloperPreview {
  page: PreviewTarget["page"];
  readonly pending: boolean;
  spread(): Spread;
  dispose(): void;
}

function replayFixture(result: BattleResult): BattleResult {
  if (result.kind !== "preview") return result;
  const attacker = result.units.find(unit => unit.side === "我方");
  const defender = result.units.find(unit => unit.side === "敌方");
  if (!attacker || !defender) throw new Error("回放样例需要双方单位。");
  const hits = [
    { actor: attacker, target: defender, damage: 2450, skill: "我方行动" },
    { actor: defender, target: attacker, damage: 890, skill: "敌方行动" },
  ];
  let units = result.units.map((unit, index) => ({ ...unit, hp: 6000, maxHp: 6000, initiative: index * 18 }));
  const log: Frame["log"] = [];
  const frames = hits.map(({ actor, target, damage, skill }, index): Frame => {
    units = units.map(unit => unit.id === target.id ? { ...unit, hp: unit.hp - damage } : unit);
    log.push([String(index + 1), `${actor.name} 对 ${target.name} 造成 ${damage} 点伤害`]);
    return { actorId: actor.id, skill, kind: "回放样例", units, targets: [{ unit: target, damage }], log: [...log] };
  });
  return { kind: "replay", frames };
}

export async function openDeveloperPreview(target: PreviewTarget, redraw: () => void, run: Run): Promise<DeveloperPreview> {
  const port = demoPort();
  const { accounts: [account] } = await port.accounts();
  if (!account) throw new Error("开发预览账号缺失。");
  if (target.page === "account") {
    const previewOnly = async () => { throw new Error("请退出开发预览后操作本地账号。"); };
    const view = accountView({
      ...port,
      inspectAccount: previewOnly, importAccount: previewOnly, generateAccount: previewOnly,
      renameAccount: previewOnly, deleteAccount: previewOnly, exportAccount: previewOnly,
      async accountCatalogue() {
        const { allies } = await port.battleCatalogue();
        return { characters: allies, lightCones: account.characters.flatMap(character => {
          const cone = character.cone;
          if (!cone) return [];
          const id = cone.image.split("/").at(-1)?.split(".")[0];
          return id ? [{ id, name: cone.name }] : [];
        }) };
      },
    }, redraw, run, () => {}, () => {});
    if (target.view !== "empty") await view.open(target.view, account);
    return { page: "account", pending: false,
      spread: () => target.view === "empty" ? view(undefined, [], []) : view(account, [account], []), dispose() {} };
  }

  if (target.page === "battle" && target.view === "replay") {
    const { initial } = await port.battleCatalogue();
    const result = replayFixture(await port.battle(initial));
    const view = battleView(redraw);
    return { page: "battle", pending: false, spread: () => view(result), dispose() {} };
  }

  const demo = demoSessionPort();
  const scenario = target.page === "match" ? target.scenario : target.view === "currencyWars" ? "preparation" : undefined;
  const session = matchSession(demo, redraw, scenario ? accountId => demo.preview({
    accountId, rulesVersion: demoRules, enemyDifficulty: 8, affixCount: 2,
    randomness: { kind: "seeded", seed: "layout" },
  }, scenario, true) : undefined);
  const battles = battleWorkspace(port, redraw, run, snapshot => previewMatchBattle(snapshot, port, account), () => session.snapshot);
  const match = matchView(session, redraw, () => run(async () => {
    await battles.enter("currencyWars");
    preview.page = "battle";
  }), demoRules, target.page === "match" ? target.panel : undefined);
  const preview: DeveloperPreview = {
    page: target.page,
    get pending() { return session.pending; },
    spread: () => preview.page === "match" ? match(account) : battles.spread(),
    dispose() { session.select(); demo.dispose(); },
  };
  try {
    await session.enter(account.id);
    if (scenario && !session.snapshot) throw new Error(session.notice || "无法打开对局预览。");
    if (target.page === "battle") await battles.enter(target.view === "currencyWars" ? "currencyWars" : "standard");
    if (target.page === "match" && target.model && session.snapshot) {
      const model = session.insights?.models[0];
      if (!model) throw new Error("演示模型不可用。");
      await session.control(session.snapshot, { kind: "model", modelId: model.id });
    }
    return preview;
  } catch (error) {
    preview.dispose();
    throw error;
  }
}
