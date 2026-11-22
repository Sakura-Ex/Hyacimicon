import type {
  Account,
  Item,
  Match,
  MatchStep,
  Combatant,
  Piece,
  PlayerPort,
  Unit,
} from "./contracts";

// Preview data identifies local artwork and leaves game descriptions and calculated values empty.
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
    portrait: `/portraits/${id}.png`,
  };
}
function item(name: string, image: string): Item {
  return { name, image, detail: "", count: 1 };
}
const gear = ["轮滑鞋", "折叠小刀", "和平手枪", "幸运星", "生命之花", "量产型装甲"].map(
  (name, i) => item(name, `/items/${350201 + i}.webp`),
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
    stats: [
      ["生命", "—"],
      ["速度", "—"],
      ["防御", "—"],
    ],
    skills: ["普攻", "战技", "终结技", "天赋", "秘技"].map((tab) => ({
      group: "角色技能",
      tab,
      level: null,
      title: null,
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
function piece(id: string, stars: number, items: Item[] = []): Piece {
  return { ...unit(id), stars, gear: items };
}
const routeKinds: MatchStep["kind"][][] = [
  ["reward", "battle", "battle", "encounter", "reward", "battle", "boss"],
  ["reward", "battle", "battle", "encounter", "battle", "reward", "boss"],
  ["reward", "battle", "encounter", "battle", "battle", "boss"],
];
const previewHealth = 68;
const previewMaxHealth = 100;
const previewGold = 37;
const initial: Match = {
  id: "preview-match",
  revision: 1,
  node: "2-3",
  planes: routeKinds.map((nodes, plane) =>
    nodes.map((kind, node) => ({ id: `${plane + 1}-${node + 1}`, kind })),
  ),
  preview: true,
  health: previewHealth,
  maxHealth: previewMaxHealth,
  streak: 2,
  interest: 3,
  capacity: 8,
  gold: previewGold,
  front: [
    piece("1205", 2, [gear[2]]),
    piece("1310", 2, gear.slice(0, 2)),
    piece("1204", 1, [gear[4]]),
    piece("1306", 1, [gear[5]]),
  ],
  back: [piece("1314", 2, [gear[3]]), piece("1409", 2, [gear[4], gear[0]]), piece("1211", 1), piece("1203", 1)],
  backCapacity: 6,
  bench: [piece("1112", 1, [gear[1]]), piece("1208", 1, [gear[5]]), piece("1212", 1), piece("1204", 1)],
  traits: [
    item("云上五骁", "/traits/100702.webp"),
    item("击破", "/traits/200502.webp"),
    item("治疗", "/traits/201002.webp"),
    item("星核猎手", "/traits/100802.webp"),
    item("召唤", "/traits/200602.webp"),
  ],
  inventory: gear,
  consumables: [
    item("精密拆装扳手", "/items/350102.webp"),
    item("冶金炉", "/items/350103.webp"),
    item("员工投影仪", "/items/350105.webp"),
  ],
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
  feedbackFields: [
    { key: "gold", label: "剩余金币", min: 0, current: previewGold },
    { key: "health", label: "整局血量", min: 0, max: previewMaxHealth, current: previewHealth },
  ],
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
    throw new Error("请使用 Windows 桌面版导入或生成账号。");
  }
  function check(id: string, revision: number) {
    if (id !== match.id || revision !== match.revision)
      throw new Error("局面版本已更新，请重新读取局面。");
  }
  return {
    async accounts() {
      return { accounts: [account], warnings: [] };
    },
    async inspectAccount() { return requireAccountService(); },
    async importAccount() {
      return requireAccountService();
    },
    async generateAccount() {
      return requireAccountService();
    },
    async accountCatalogue() { return requireAccountService(); },
    async renameAccount() { return requireAccountService(); },
    async deleteAccount() { return requireAccountService(); },
    async exportAccount() { return requireAccountService(); },
    async match(accountId) {
      if (accountId !== account.id) throw new Error("请切换到预览账号查看示例对局。");
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
      const health = input.observed.health;
      if (
        !match.actions.some((a) => a.id === input.action) ||
        !Number.isSafeInteger(gold) ||
        gold < 0
      )
        throw new Error("请选择实际操作并填写有效的金币数量。");
      if (health !== undefined && (!Number.isSafeInteger(health) || health < 0 || health > previewMaxHealth)) {
        throw new Error("请填写有效的整局血量。");
      }
      match = {
        ...match,
        revision: match.revision + 1,
        gold,
        health: health ?? match.health,
        feedbackFields: match.feedbackFields.map((f) => ({
          ...f,
          current: input.observed[f.key] ?? f.current,
        })),
        lastAction: input.action,
        advice: [],
        refresh: undefined,
      };
      return match;
    },
  };
}
