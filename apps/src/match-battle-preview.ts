import { combatant } from "./demo";
import { matchCatalogue, zoneNames } from "./match-catalogue";
import type { Account, BattlePort, BattleResult } from "./contracts";
import type { MatchSnapshot } from "./match-session-contract";

export async function previewMatchBattle(snapshot: MatchSnapshot, port: BattlePort, account?: Account): Promise<BattleResult> {
  const p = snapshot.preparation;
  if (!p) throw new Error("请先在对局中完成开局准备。");
  const c = matchCatalogue(snapshot, account);
  const allies = p.board.pieces.filter(piece => piece.slot.zone !== "bench").map(piece =>
    combatant({ ...c.unit(piece.unitId), id: piece.pieceId }, zoneNames[piece.slot.zone],
      piece.equipment.map(item => ({ ...c.item(item.definitionId), count: item.count }))));
  const { enemies } = await port.battleCatalogue();
  const revealed = p.revealedEnemies.map(enemy =>
    combatant(enemies.find(unit => unit.id === enemy.id) ?? { ...enemy, image: "" }, "敌方"));
  return { kind: "preview", units: [...allies, ...revealed] };
}
