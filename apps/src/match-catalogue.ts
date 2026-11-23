import type { Account, Item, Unit } from "./contracts.ts";
import { demoMatchCatalogue, demoRules } from "./demo.ts";
import type { MatchAction, MatchSnapshot, Slot, VisibleMechanism } from "./match-session-contract.ts";
import type { MatchInsights } from "./match-port.ts";

export const zoneNames = { front: "前台", back: "后台", bench: "备战席" };
export const crystalNames = { white: "白", blue: "蓝", gold: "金", prismatic: "彩" };
export const mechanismNames = {
  aha: "阿哈", wishPower: "幻月游戏·愿力", trailblazeShop: "开拓商店", chimeraGarden: "奇兽花园",
} satisfies Record<VisibleMechanism["kind"], string>;

export interface Catalogue {
  units: Record<string, Unit & { traitIds?: string[] }>;
  items: Record<string, Omit<Item, "count"> & { kind: "equipment" | "consumable"; component?: boolean }>;
  traits: Record<string, { name: string; image: string; detail?: string }>;
}

export function matchCatalogue(s: MatchSnapshot, account?: Account, insights?: MatchInsights) {
  const catalogue = insights?.catalogue ?? (s.rulesVersion === demoRules ? demoMatchCatalogue() : undefined);
  const byPiece = new Map(s.preparation?.board.pieces.map(piece => [piece.pieceId, piece]));
  const byItem = new Map([...s.preparation?.inventory ?? [],
    ...s.preparation?.board.pieces.flatMap(piece => piece.equipment) ?? []].map(item => [item.itemId, item.definitionId]));
  function unit(id: string) {
    return catalogue?.units[id] ?? account?.characters.find(c => c.id === id) ?? { id, name: id, image: "" };
  }
  function item(id: string) {
    return catalogue?.items[id] ?? { name: id, image: "", detail: "", kind: undefined, component: false };
  }
  function trait(id: string) {
    return catalogue?.traits[id] ?? { name: id, image: "" };
  }
  const pieceName = (id: string) => {
    const piece = byPiece.get(id);
    return piece ? unit(piece.unitId).name : id;
  };
  const itemName = (id: string) => item(byItem.get(id) ?? id).name;
  const slotName = (slot: Slot) => `${zoneNames[slot.zone]} ${slot.index + 1}`;
  function actionLabel(action: MatchAction): string {
    switch (action.kind) {
      case "buy": {
        const offer = s.preparation?.shop.slots.find(offer => offer?.offerId === action.offerId);
        return offer ? "招募 " + unit(offer.unitId).name : "招募角色";
      }
      case "sell": return "出售 " + pieceName(action.pieceId);
      case "move": return `将 ${pieceName(action.pieceId)} 移至${slotName(action.destination)}`;
      case "equip": return `将 ${itemName(action.itemId)} 装备给 ${pieceName(action.pieceId)}`;
      case "combine": return `合成 ${itemName(action.itemId)} 与 ${itemName(action.otherItemId)}`;
      case "useItem": return `对 ${action.target.kind === "piece" ? pieceName(action.target.pieceId) : itemName(action.target.itemId)} 使用 ${itemName(action.itemId)}`;
      case "lockShop": return action.locked ? "锁定商店" : "解锁商店";
      case "refreshShop": return "刷新商店";
      case "buyExperience": return "购买经验";
      case "openCrystal": return "开启晶矿";
      case "refreshChoice": return "刷新候选";
      case "refreshSupply": return "刷新补给";
      case "returnToPreparation": return "返回备战";
      case "returnToSelection": return "回到选择";
      case "chooseInvestmentEnvironment": return "选择投资环境";
      case "chooseInvestmentStrategy": return "选择投资策略";
      case "takeSupply": return "领取补给";
      case "openSpotlight": return "选择主角光环";
      case "chooseSpotlight": return "为 " + pieceName(action.pieceId) + " 选择光环";
      case "chooseTeaBreak": return "选择茶歇";
      case "claimTeaBreakReward": return "领取茶歇奖励";
      case "chooseMechanism": return "确认羁绊选项";
      case "openTraitChoice": return "打开羁绊选择";
      case "selectTraitOption": return "改选羁绊";
      case "confirmTraitChoice": return "确认羁绊";
      case "startBattle": return "开始战斗";
      case "restartBattle": return "重开战斗";
      case "leaveBattle": return "暂离战斗";
      case "reenterBattle": return "重新进入战斗";
    }
  }
  return { unit, item, trait, pieceName, actionLabel };
}
export type MatchCatalogue = ReturnType<typeof matchCatalogue>;
