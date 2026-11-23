import type { MatchCommand } from "./contracts";

export type DropTarget =
  | { pieceId: string; itemId?: never }
  | { itemId: string; pieceId?: string };

export function commandsForDrop(commands: MatchCommand[], itemId: string, target: DropTarget) {
  if (target.itemId === itemId) return [];
  return commands.filter(({ operation }) => {
    if (operation.kind === "equip") {
      return !target.itemId && operation.itemId === itemId && operation.pieceId === target.pieceId;
    }
    if (operation.kind === "combine") {
      return (operation.itemId === itemId && operation.otherItemId === target.itemId) ||
        (operation.otherItemId === itemId && operation.itemId === target.itemId);
    }
    if (operation.kind === "useItem") {
      return operation.itemId === itemId && operation.targetItemId === target.itemId &&
        (operation.pieceId === undefined || operation.pieceId === target.pieceId) &&
        (operation.pieceId !== undefined || operation.targetItemId !== undefined);
    }
    return false;
  });
}
