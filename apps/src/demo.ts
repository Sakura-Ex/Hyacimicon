import type { Catalogue } from "./match-catalogue";
import type {
  Account,
  Item,
  AccountPort,
  BattlePort,
  Combatant,
  Unit,
} from "./contracts";

export const demoRules = "demo-4.7";

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
export interface DemoItem extends Item { itemId: string }
interface DemoPiece extends Unit {
  pieceId: string;
  slot: number;
  stars: number;
  gear: DemoItem[];
}
function piece(id: string, stars: number, items: Item[] = []): DemoPiece {
  return { ...unit(id), pieceId: "", slot: 0, stars, gear: matchItems("gear", items) };
}
function matchItems(prefix: string, items: Item[]): DemoItem[] {
  return items.map((item, i) => ({ ...item, itemId: `${prefix}-${i}` }));
}
const traitRows: [string, number, number, number[]][] = [
  ["云上五骁", 100702, 3, [2, 4, 5]],
  ["星核猎手", 100802, 2, [2, 4, 6]],
  ["击破", 200502, 5, [2, 4, 6, 8, 10]],
  ["召唤", 200602, 4, [2, 4, 7]],
  ["开拓银轨", 100502, 2, [2, 4, 6]],
  ["千星商会", 100602, 1, [2, 3, 4]],
  ["幻月游戏", 100302, 2, [1, 3, 5]],
  ["黄金大饭店", 101202, 3, [2, 4, 6]],
  ["梦境交织", 101002, 1, [2, 4, 6]],
  ["治疗", 201002, 2, [2, 3]],
  ["群攻", 200302, 3, [2, 4, 7]],
  ["追击", 200102, 2, [2, 4, 7]],
  ["持续伤害", 200202, 1, [2, 4, 6]],
  ["连击", 200402, 2, [2, 4, 6]],
];
const previewTraits = traitRows.map(([name, icon, count, thresholds]) => ({
  ...item(name, `/traits/${icon}.webp`),
  progress: { count, thresholds },
}));
const offerTraitIcons: Record<string, number> = {
  ...Object.fromEntries(traitRows.map(([name, icon]) => [name, icon])),
  黑市主理人: 300402, 小小巨人: 100902, 欢愉: 200702, 星间旅人: 100402,
  少女乐队: 101102, 奇兽花园: 100202, 反击: 200802,
};
const offerTraitNames: Record<string, string[]> = {
  "1204": ["云上五骁", "召唤"],
  "1005": ["星核猎手", "持续伤害"],
  "1314": ["千星商会", "群攻", "黑市主理人"],
  "1306": ["小小巨人", "梦境交织", "欢愉"],
  "1203": ["星间旅人", "治疗"],
  "1308": ["梦境交织", "群攻"],
  "1309": ["少女乐队", "追击"],
  "1409": ["奇兽花园", "召唤", "治疗"],
  "1205": ["云上五骁", "反击"],
  "1310": ["星核猎手", "击破"],
};
const initial = {
  health: 68,
  streak: 2,
  capacity: 8,
  gold: 37,
  front: [
    piece("1205", 2, [gear[2]]),
    piece("1310", 2, gear.slice(0, 2)),
    piece("1204", 1, [gear[4]]),
    piece("1306", 1, [gear[5]]),
  ],
  frontCapacity: 4,
  back: [piece("1314", 2, [gear[3]]), piece("1409", 2, [gear[4], gear[0]]), piece("1211", 1), piece("1203", 1)],
  backCapacity: 6,
  bench: [piece("1112", 1, [gear[1]]), piece("1208", 1, [gear[5]]), piece("1212", 1), piece("1204", 1)],
  benchCapacity: 9,
  traits: [0, 2, 9, 1, 3].map(index => previewTraits[index]),

  inventory: matchItems("stock", gear),
  consumables: matchItems("supply", [
    item("精密拆装扳手", "/items/350102.webp"),
    item("冶金炉", "/items/350103.webp"),
    item("员工投影仪", "/items/350105.webp"),
  ]),
  offers: ["1204", "1005", "1314", "1306", "1203"].map((id, i) => ({
    offerId: "offer-" + i,
    unit: unit(id),
    cost: [4, 2, 3, 4, 2][i],


  })),
};
const previewCatalogue: Catalogue = {
  units: Object.fromEntries(Object.keys(names).map(id => [id, {
    ...unit(id), traitIds: (offerTraitNames[id] ?? []).map(name => String(offerTraitIcons[name])),
  }])),
  items: {
    ...Object.fromEntries([...initial.inventory, ...initial.consumables].map(entry => {
      const component = entry.itemId.startsWith("stock");
      return [entry.image.slice(7, -5), { ...entry, kind: component ? "equipment" : "consumable", component }];
    })),
    "demo-combined": { name: "合成装备", image: "", kind: "equipment", detail: "演示进阶装备。" },
  },
  traits: Object.fromEntries(Object.entries(offerTraitIcons).map(([name, id]) => [String(id), { name, image: "/traits/" + id + ".webp" }])),
};
export const demoMatchCatalogue = () => previewCatalogue;
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
export function combatant(u: Unit, side: string, gear: Item[] = []): Combatant {
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
const zones = ["front", "back", "bench"] as const;

export function demoFixture(scenario?: string) {
  let match = structuredClone(initial);
  if (scenario === "full") {
    match.backCapacity = 9;
    match.back.push(piece("1308", 1, [gear[0], gear[3], gear[4]]), piece("1309", 2, [gear[3]]),
      piece("1005", 1, [gear[1], gear[2]]), piece("1407", 2, [gear[4], gear[5]]), piece("1304", 1, [gear[5]]));
    match.bench.push(piece("1004", 1, [gear[5]]), piece("1310", 1),
      piece("1205", 1, [gear[0], gear[2], gear[5]]), piece("1314", 1, [gear[4]]), piece("1306", 1));
    match.capacity = match.front.length + match.back.length;
    match.traits = structuredClone(previewTraits);
  }
  for (const zone of zones) {
    match[zone].forEach((p, slot) => {
      p.slot = slot;
      p.pieceId = `${zone}-${slot}`;
      p.gear = matchItems(p.pieceId, p.gear);
    });
  }
  if (scenario === "stock") {
    match.inventory = matchItems("stock", Array.from({ length: 30 }, (_, i) => gear[i % gear.length]));
  }
  return match;
}
export type DemoFixture = ReturnType<typeof demoFixture>;

export function demoPort(): AccountPort & BattlePort {
  function requireAccountService(): never {
    throw new Error("请使用 Windows 桌面版导入或生成账号。");
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
      throw new Error("请从当前对局打开货币战争战斗预览。");
    },
  };
}
