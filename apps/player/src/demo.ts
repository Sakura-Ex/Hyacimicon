import type {
  Account,
  Item,
  Match,
  Combatant,
  Piece,
  PlayerPort,
  Unit,
} from "./contracts";

// The catalogue identifies bundled artwork. Preview data contains no game descriptions or calculated values.
const names: Record<string, string> = {
  "1409": "风堇",
  "1310": "流萤",
  "1204": "景元",
  "1203": "罗刹",
  "1314": "翡翠",
  "1308": "黄泉",
  "1309": "知更鸟",
  "1005": "卡芙卡",
  "1205": "刃",
  "1306": "花火",
  "1004": "瓦尔特",
  "1212": "镜流",
  "1208": "符玄",
  "1407": "遐蝶",
  "1211": "白露",
  "1304": "砂金",
  "1112": "托帕",
};
function unit(id: string): Unit {
  return {
    id,
    name: names[id],
    image: "/avatars/head-" + id + ".png",
    portrait: ["1409", "1204"].includes(id)
      ? `/portraits/${id}.png`
      : undefined,
  };
}
function item(name: string, image: string): Item {
  return { name, image, detail: "", count: 1 };
}
const gear = ["350201", "350202", "350203", "350204", "350205", "350206"].map(
  (id) => item("装备 " + id, "/items/" + id + ".webp"),
);
const account: Account = {
  id: "preview",
  name: "预览账号",
  source: "界面预览",
  characters: [
    "1409",
    "1310",
    "1204",
    "1203",
    "1314",
    "1308",
    "1309",
    "1005",
    "1205",
    "1306",
    "1004",
    "1212",
    "1208",
    "1407",
    "1211",
    "1304",
  ].map((id) => ({
    ...unit(id),
    path: "",
    level: null,
    ascension: null,
    eidolons: null,
    portrait: ["1409", "1204"].includes(id)
      ? "/portraits/" + id + ".png"
      : undefined,
    stats: [
      ["生命", "—"],
      ["速度", "—"],
      ["防御", "—"],
    ],
    skills: ["普攻", "战技", "终结技", "天赋", "秘技"].map((tab) => ({
      group: "角色技能",
      tab,
      level: null,
      title: tab,
      description: "",
      scaling: [],
    })),
    cone:
      id === "1409" ? item("光锥 23042", "/light-cones/23042.png") : undefined,
    relics:
      id === "1409"
        ? ["124_0", "124_1", "124_2", "124_3", "320_0", "320_1"].map((key, i) =>
            item(
              ["头部", "手部", "躯干", "脚部", "位面球", "连结绳"][i],
              "/relics/" + key + ".png",
            ),
          )
        : [],
  })),
};
function piece(id: string, items: Item[] = []): Piece {
  return { ...unit(id), stars: null, gear: items };
}
const initial: Match = {
  id: "preview-match",
  revision: 1,
  node: "预览",
  health: null,
  streak: null,
  interest: null,
  capacity: 8,
  gold: null,
  front: [
    piece("1205", [gear[2]]),
    piece("1310", gear.slice(0, 2)),
    piece("1204", [gear[4]]),
    piece("1306", [gear[5]]),
  ],
  back: ["1314", "1409", "1211", "1203"].map((id) => piece(id)),
  backCapacity: 6,
  bench: ["1112", "1208", "1212", "1204"].map((id) => piece(id)),
  traits: ["100702", "200502", "201002", "100802", "200602"].map((id) =>
    item("羁绊 " + id, "/traits/" + id + ".webp"),
  ),
  inventory: gear,
  consumables: ["350102", "350103", "350105"].map((id) =>
    item("消耗品 " + id, "/items/" + id + ".webp"),
  ),
  offers: ["1204", "1005", "1314", "1306", "1203"].map((id) => ({
    unit: unit(id),
    cost: null,
    note: "",
  })),
  advice: [],
  actions: [
    ["recruit", "招募"],
    ["hold", "保留金币"],
    ["refresh", "刷新商店"],
  ].map(([id, title]) => ({
    id,
    title,
    expected: {},
  })),
  refresh: { actionId: "refresh", label: "刷新" },
  feedbackFields: [{ key: "gold", label: "剩余金币", min: 0, current: null }],
  locked: false,
};
const enemies = ["1002030", "1002040"].map((id, i) => ({
  id,
  name: "敌方单位 " + (i + 1),
  image: "/enemies/Monster_" + id + ".webp",
}));
const allies = Object.keys(names).map(unit);
function resolveUnits(ids: string[], options: Unit[]): Unit[] {
  if (!ids.length || new Set(ids).size !== ids.length)
    throw new Error("请为双方选择参战单位，每个单位选择一次。");
  return ids.map((id) => {
    const entry = options.find((u) => u.id === id);
    if (!entry) throw new Error("未知参战单位 " + id);
    return entry;
  });
}
function combatant(u: Unit, side: string, gear: Item[] = []): Combatant {
  return {
    ...u,
    side,
    gear,
    element: "",
    hp: null,
    maxHp: null,
    energy: null,
    maxEnergy: null,
    stats: [
      ["攻击", "—"],
      ["防御", "—"],
      ["当前速度", "—"],
    ],
    buffs: [],
    initiative: null,
  };
}
export function demoPort(): PlayerPort {
  let match = structuredClone(initial);
  function requireAccountService(): never {
    throw new Error("此操作需要接入账号服务。");
  }
  function check(id: string, revision: number) {
    if (id !== match.id || revision !== match.revision)
      throw new Error("局面版本已更新，请重新读取局面。");
  }
  return {
    async accounts() {
      return [account];
    },
    async importAccount() {
      return requireAccountService();
    },
    async generateAccount() {
      return requireAccountService();
    },
    async match(accountId) {
      if (accountId !== account.id) throw new Error("找不到预览账号。");
      return match;
    },
    async battleCatalogue() {
      return {
        allies,
        enemies,
        initial: {
          mode: "standard",
          allyIds: ["1204", "1310"],
          enemyIds: [enemies[0].id],
        },
      };
    },
    async battle(input) {
      if (input.mode === "standard") {
        if (input.allyIds.length > 4)
          throw new Error("常规战斗最多选择四名角色。");
        return {
          kind: "preview",
          units: [
            ...resolveUnits(input.allyIds, allies).map((u) =>
              combatant(u, "我方"),
            ),
            ...resolveUnits(input.enemyIds, enemies).map((u) =>
              combatant(u, "敌方"),
            ),
          ],
        };
      }
      check(input.matchId, input.revision);
      const deployed = (pieces: Piece[], side: string) =>
        pieces.map((p, i) =>
          combatant({ ...p, id: side + "-" + i }, side, p.gear),
        );
      return {
        kind: "preview",
        units: [
          ...deployed(match.front, "前台"),
          ...deployed(match.back, "后台"),
          ...enemies.map((u) => combatant(u, "敌方")),
        ],
      };
    },
    async setShopLock(id, revision, locked) {
      check(id, revision);
      match = { ...match, locked, revision: revision + 1 };
      return match;
    },
    async saveFeedback(input) {
      check(input.matchId, input.revision);
      const gold = input.observed.gold;
      if (
        !match.actions.some((a) => a.id === input.action) ||
        !Number.isSafeInteger(gold) ||
        gold < 0
      )
        throw new Error("请选择实际操作并填写有效的金币数量。");
      match = {
        ...match,
        revision: match.revision + 1,
        gold,
        feedbackFields: match.feedbackFields.map((f) => ({
          ...f,
          current: gold,
        })),
        lastAction: input.action,
        advice: [],
        refresh: undefined,
      };
      return match;
    },
  };
}
