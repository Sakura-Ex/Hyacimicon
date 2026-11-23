import type {
  Account,
  Item,
  Match,
  MatchCommand,
  MatchItem,
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
  return { ...unit(id), pieceId: "", slot: 0, stars, gear: matchItems("gear", items) };
}
function matchItems(prefix: string, items: Item[]): MatchItem[] {
  return items.map((item, i) => ({ ...item, itemId: `${prefix}-${i}` }));
}
const routeKinds: MatchStep["kind"][][] = [
  ["reward", "battle", "battle", "encounter", "reward", "battle", "boss"],
  ["reward", "battle", "battle", "encounter", "battle", "reward", "boss"],
  ["reward", "battle", "encounter", "battle", "battle", "boss"],
];
const previewHealth = 68;
const previewMaxHealth = 100;
const previewGold = 37;
const previewExperienceCost = 4;
const previewExperienceGain = 4;
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
const offerTraits = (id: string) =>
  (offerTraitNames[id] ?? []).map(name => ({ name, image: `/traits/${offerTraitIcons[name]}.webp` }));
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
  overview: {
    opponent: {
      name: "凛冬经贸联合体", difficulty: 14, stars: 2,
      enemies: [{ name: "银鬃尉官", stars: 2 }, { name: "银鬃炮手", stars: 1 }, { name: "银鬃近卫", stars: 1 }],
    },
    modifiers: [
      { name: "随从强化", detail: "普通和精英敌人速度提高 30%，生命上限提高 20%。" },
      { name: "第二位面强化", detail: "第二位面敌人速度提高 40%，生命上限提高 25%。" },
    ],
    investmentEnvironment: { name: "增发货币", detail: "第一、第二、第三阶段开始时，分别获得一个价值 6、8、10 金币的晶矿。" },
    investmentStrategies: [{ name: "概率事件", detail: "刷新时有 50% 概率获得一次免费刷新。" }],
    crystals: { count: 2, entries: [{ name: "晶矿", detail: "6 金币" }, { name: "晶矿", detail: "8 金币" }] },
  },
  capacity: 8,
  gold: previewGold,
  experience: { level: 8, current: 68, required: 72 },
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
  traitMechanisms: ["aha", "wishPower", "trailblazeShop", "chimeraGarden"],
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
    note: "",
    traits: offerTraits(id),
  })),
  advice: [],
  commands: [],
  actions: [
    ["recruit", "招募"],
    ["hold", "保留金币"],
    ["refresh", "刷新商店"],
  ].map(([id, title]) => ({
    id,
    title,
    expected: {},
  })),
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
const zones = ["front", "back", "bench"] as const;

function previewCommands(m: Match): MatchCommand[] {
  const commands: MatchCommand[] = [];
  const add = (operation: MatchCommand["operation"], label: string, detail?: string) => {
    commands.push({ id: String(commands.length), operation, label, detail });
  };
  const pieces = zones.flatMap(zone => m[zone]);
  const location = (p: Piece) => `${p.pieceId.startsWith("front-") ? "前台" : p.pieceId.startsWith("back-") ? "后台" : "备战席"} ${p.slot + 1}`;
  for (const item of m.inventory) {
    for (const p of pieces.filter(p => p.gear.length < 3)) {
      add({ kind: "equip", itemId: item.itemId, pieceId: p.pieceId }, `装备给 ${p.name}`, `${location(p)} · ${p.gear.length} / 3 槽位已用`);
    }
  }
  for (const item of m.consumables) {
    if (item.name === "精密拆装扳手") {
      for (const p of pieces.filter(p => p.gear.length)) {
        add({ kind: "useItem", itemId: item.itemId, pieceId: p.pieceId }, `卸下 ${p.name} 的装备`, location(p));
      }
    }
  }
  for (const offer of m.offers) {
    if (m.bench.length < m.benchCapacity && offer.cost !== null && offer.cost <= (m.gold ?? 0)) {
      add({ kind: "buy", offerId: offer.offerId }, `招募 ${offer.unit.name}`, `${offer.cost} 金币`);
    }
  }
  add({ kind: "lockShop", locked: !m.locked }, m.locked ? "解锁" : "锁定");
  if ((m.gold ?? 0) >= 2) add({ kind: "refreshShop" }, "刷新 2", "2 金币");
  if ((m.gold ?? 0) >= previewExperienceCost && m.experience?.required != null) {
    add({ kind: "buyExperience" }, `购买经验 · ${previewExperienceCost}`, `${previewExperienceCost} 金币获得 ${previewExperienceGain} 经验`);
  }
  if (m.event?.kind === "aura") {
    for (const p of [...m.front, ...m.back]) {
      add({ kind: "chooseAura", eventId: m.event.id, pieceId: p.pieceId, optionId: "demo-aura" }, p.name, "舞台聚光 · 示例光环");
    }
  }
  if (m.event?.kind === "teaBreak") {
    add({ kind: "teaBreak", eventId: m.event.id, optionId: "demo-supply" }, "领取随行补给", "精密拆装扳手 ×1");
  }
  return commands;
}

export function demoPort(scenario?: string | null): PlayerPort {
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
  match.companion = { name: "绷带猫", detail: "装备补给" };
  if (scenario === "aura") {
    match.node = "2-7";
    match.event = { id: "opening-act", kind: "aura", title: "主角光环", detail: "为出战角色选择本场首领战的光环。" };
  } else if (scenario === "teaBreak") {
    match.node = "2-6";
    match.event = { id: "intermission", kind: "teaBreak", title: "幕后茶歇", detail: "绷带猫带来了随行补给。" };
  } else if (scenario === "stock") {
    match.inventory = matchItems("stock", Array.from({ length: 30 }, (_, i) => gear[i % gear.length]));
  }
  match.commands = previewCommands(match);
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
    async executeMatchCommand({ matchId, revision, commandId }) {
      check(matchId, revision);
      const operation = match.commands.find(command => command.id === commandId)?.operation;
      if (!operation) throw new Error("该操作当前不可用。");
      const next = structuredClone(match);
      const pieces = zones.flatMap(zone => next[zone]);
      if (operation.kind === "equip") {
        const target = pieces.find(p => p.pieceId === operation.pieceId)!;
        const item = next.inventory.find(i => i.itemId === operation.itemId)!;
        target.gear.push(item);
        next.inventory = next.inventory.filter(i => i !== item);
      } else if (operation.kind === "useItem") {
        const target = pieces.find(p => p.pieceId === operation.pieceId)!;
        next.inventory.push(...target.gear);
        target.gear = [];
        next.consumables = next.consumables.filter(i => i.itemId !== operation.itemId);
      } else if (operation.kind === "buy") {
        const offer = next.offers.find(o => o.offerId === operation.offerId)!;
        const recruit = piece(offer.unit.id, 1);
        recruit.slot = next.bench.length;
        recruit.pieceId = "recruit-" + revision;
        next.bench.push(recruit);
        next.gold = (next.gold ?? 0) - offer.cost!;
        next.offers = next.offers.filter(o => o !== offer);
      } else if (operation.kind === "refreshShop") {
        next.gold = (next.gold ?? 0) - 2;
        const ids = ["1308", "1309", "1409", "1205", "1310"];
        next.offers = initial.offers.map((offer, i) => {
          const id = ids[(i + revision) % ids.length];
          return { ...offer, unit: unit(id), traits: offerTraits(id) };
        });
      } else if (operation.kind === "buyExperience") {
        const experience = next.experience!;
        next.gold = (next.gold ?? 0) - previewExperienceCost;
        experience.current += previewExperienceGain;
        if (experience.required !== null && experience.current >= experience.required) {
          experience.current -= experience.required;
          experience.level++;
          experience.required = experience.level === 9 ? 84 : null;
          next.capacity++;
        }
      } else if (operation.kind === "lockShop") {
        next.locked = operation.locked;
      } else if (operation.kind === "chooseAura") {
        next.aura = { pieceId: operation.pieceId, name: "舞台聚光", detail: "示例光环" };
        delete next.event;
      } else if (operation.kind === "teaBreak") {
        next.consumables.push({ ...initial.consumables[0], itemId: "tea-supply" });
        delete next.event;
      }
      next.revision++;
      next.feedbackFields = next.feedbackFields.map(field => field.key === "gold" ? { ...field, current: next.gold } : field);
      next.commands = previewCommands(next);
      match = next;
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
      };
      match.commands = previewCommands(match);
      return match;
    },
  };
}
