import type { LegalAction, Slot } from "./match-session-contract.ts";

export type DragSource = { kind: "item" | "piece"; id: string };
export type MatchDrop = { pieceId?: string; itemId?: string; slot?: Slot };

export function actionsForDrop(actions: readonly LegalAction[], source: DragSource, target: MatchDrop) {
  return actions.filter(({ action }) => {
    if (source.kind === "piece") {
      return action.kind === "move" && action.pieceId === source.id && target.slot?.zone === action.destination.zone &&
        target.slot.index === action.destination.index;
    }
    if (target.itemId === source.id) return false;
    switch (action.kind) {
      case "equip": return !target.itemId && action.itemId === source.id && action.pieceId === target.pieceId;
      case "combine": return action.itemId === source.id && action.otherItemId === target.itemId ||
        action.otherItemId === source.id && action.itemId === target.itemId;
      case "useItem": return action.itemId === source.id && (target.itemId
        ? action.target.kind === "item" && action.target.itemId === target.itemId
        : action.target.kind === "piece" && action.target.pieceId === target.pieceId);
      default: return false;
    }
  });
}
