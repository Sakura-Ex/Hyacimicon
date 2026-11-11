"""商店、角色牌池、升级和角色合成规则。"""

import random
from dataclasses import dataclass, field

from .models import GameRules, ShopOffer, UnitDefinition, UnitInstance


class MarketActionError(ValueError):
    """商店操作不满足资源或阶段条件。"""


@dataclass
class Roster:
    """按角色和星级保存已拥有角色。"""

    units: dict[tuple[str, int], int] = field(default_factory=dict)

    def add(self, unit_id: str, *, star: int = 1) -> tuple[str, ...]:
        """加入一个角色并递归合成三星同名角色。"""

        key = (unit_id, star)
        self.units[key] = self.units.get(key, 0) + 1
        merges: list[str] = []
        while self.units.get(key, 0) >= 3 and star < 4:
            self.units[key] -= 3
            if self.units[key] == 0:
                del self.units[key]
            star += 1
            key = (unit_id, star)
            self.units[key] = self.units.get(key, 0) + 1
            merges.append(f"{unit_id}:{star}")
        return tuple(merges)

    def remove(self, unit_id: str, star: int) -> int:
        """移除一个角色并返回需要归还牌池的基础卡数量。"""

        key = (unit_id, star)
        count = self.units.get(key, 0)
        if count == 0:
            raise MarketActionError("没有可出售的指定角色")
        if count == 1:
            del self.units[key]
        else:
            self.units[key] = count - 1
        return 3 ** (star - 1)

    def instances(self) -> tuple[UnitInstance, ...]:
        """展开全部角色实例。"""

        return tuple(
            UnitInstance(unit_id, star)
            for (unit_id, star), count in sorted(self.units.items())
            for _ in range(count)
        )


class Market:
    """维护不可见的角色牌池和策略可见的商店。"""

    def __init__(
        self,
        rules: GameRules,
        units: dict[str, UnitDefinition],
        rng: random.Random,
    ) -> None:
        self.rules = rules
        self.units = units
        self.rng = rng
        self.pool = {
            unit_id: rules.card_copies_by_cost[unit.cost - 1] for unit_id, unit in units.items()
        }
        self.roster = Roster()
        self.level = rules.starting_level
        self.xp = 0
        self.free_rerolls = 0
        self.offers: tuple[ShopOffer, ...] = ()
        self.locked = False
        self.purchase_lock_nodes = 0
        self.free_reroll_chance = 0.0
        self.total_paid_rerolls = 0
        self.offers = ()

    @property
    def active_capacity(self) -> int:
        """当前等级可上阵的角色数。"""

        return self.level

    @property
    def xp_to_next(self) -> int | None:
        """升至下一级仍需的经验。"""

        if self.level >= 10:
            return None
        return self.rules.xp_to_next_level[self.level - 1]

    def start_node(self) -> None:
        """节点开始时自动刷新商店，锁定商店时保留原有卡牌。"""

        if self.purchase_lock_nodes > 0:
            self.purchase_lock_nodes -= 1
        if not self.locked:
            self._roll_shop()

    def set_locked(self, locked: bool) -> None:
        """设置商店锁定状态。"""

        self.locked = locked

    def refresh_with_resources(self, gold: int) -> tuple[int, bool]:
        """消耗免费刷新或金币刷新，返回剩余金币和是否计费。"""

        if self.purchase_lock_nodes > 0:
            raise MarketActionError("当前投资策略禁用了商店购买和刷新")
        paid = False
        if self.free_rerolls > 0:
            self.free_rerolls -= 1
        else:
            if gold < self.rules.refresh_cost:
                raise MarketActionError("金币不足，无法刷新商店")
            gold -= self.rules.refresh_cost
            paid = True
            self.total_paid_rerolls += 1
        self._roll_shop()
        if paid and self.free_reroll_chance > 0:
            if self.rng.random() < self.free_reroll_chance:
                self.free_rerolls += 1
        return gold, paid

    def buy_unit(self, offer_index: int, gold: int) -> tuple[int, tuple[str, ...]]:
        """购买一个商店角色，返回剩余金币和本次合成记录。"""

        if self.purchase_lock_nodes > 0:
            raise MarketActionError("当前投资策略禁用了商店购买和刷新")
        if offer_index < 0 or offer_index >= len(self.offers):
            raise MarketActionError("商店位置不存在")
        offer = self.offers[offer_index]
        if offer.unit_id is None:
            raise MarketActionError("该商店位置没有角色")
        unit = self.units[offer.unit_id]
        if gold < unit.cost:
            raise MarketActionError("金币不足，无法购买该角色")
        gold -= unit.cost
        self.pool[unit.unit_id] -= 1
        merges = self.roster.add(unit.unit_id)
        offers = list(self.offers)
        offers[offer_index] = ShopOffer(None)
        self.offers = tuple(offers)
        return gold, merges

    def sell_unit(self, unit_id: str, star: int, gold: int) -> int:
        """出售一个角色，卡片回到牌池，返回剩余金币。"""

        unit = self.units.get(unit_id)
        if unit is None:
            raise MarketActionError("未知角色")
        if star < 1 or star > 4:
            raise MarketActionError("角色星级必须位于 1 到 4")
        returned_cards = self.roster.remove(unit_id, star)
        self.pool[unit_id] += returned_cards
        return gold + unit.cost * returned_cards

    def buy_xp(
        self,
        gold: int,
        *,
        purchase_count: int = 1,
        cost_adjustment: int = 0,
        extra_cost: int = 0,
    ) -> tuple[int, tuple[int, ...]]:
        """购买经验并返回剩余金币及本次升级记录。"""

        if purchase_count < 1:
            raise MarketActionError("经验购买次数必须大于 0")
        if self.level >= 10:
            raise MarketActionError("角色等级已达到上限")
        unit_cost = max(0, self.rules.xp_purchase_cost + cost_adjustment + extra_cost)
        if gold < unit_cost * purchase_count:
            raise MarketActionError("金币不足，无法购买经验")
        next_gold = gold
        next_xp = self.xp
        next_level = self.level
        levels: list[int] = []
        for _ in range(purchase_count):
            if next_level >= 10:
                break
            next_gold -= unit_cost
            next_xp += self.rules.xp_purchase_amount
            while next_level < 10:
                required = self.rules.xp_to_next_level[next_level - 1]
                if next_xp < required:
                    break
                next_xp -= required
                next_level += 1
                levels.append(next_level)
        self.xp = next_xp
        self.level = next_level
        return next_gold, tuple(levels)

    def grant_xp(self, amount: int) -> tuple[int, ...]:
        """免费增加经验，返回本次达到的等级。"""

        if amount < 0:
            raise MarketActionError("经验值不能为负数")
        self.xp += amount
        levels: list[int] = []
        while self.level < 10:
            required = self.rules.xp_to_next_level[self.level - 1]
            if self.xp < required:
                break
            self.xp -= required
            self.level += 1
            levels.append(self.level)
        return tuple(levels)

    def _roll_shop(self) -> None:
        odds = self.rules.shop_odds[min(self.level, len(self.rules.shop_odds)) - 1]
        offers: list[ShopOffer] = []
        for _ in range(self.rules.shop_size):
            cost = self.rng.choices((1, 2, 3, 4, 5), weights=odds, k=1)[0]
            candidates = [
                unit_id
                for unit_id, remaining in self.pool.items()
                if remaining > 0 and self.units[unit_id].cost == cost
            ]
            if not candidates:
                offers.append(ShopOffer(None))
                continue
            weights = tuple(self.pool[unit_id] for unit_id in candidates)
            offers.append(ShopOffer(self.rng.choices(candidates, weights=weights, k=1)[0]))
        self.offers = tuple(offers)

    def take_random_units(
        self,
        count: int,
        *,
        cost: int | None = None,
    ) -> tuple[UnitInstance, ...]:
        """免费加入角色，用于开局补给或投资环境。"""

        candidates = [
            unit_id
            for unit_id, unit in self.units.items()
            if self.pool[unit_id] > 0 and (cost is None or unit.cost == cost)
        ]
        if not candidates:
            raise MarketActionError("角色牌池没有符合条件的角色")
        result = []
        for _ in range(count):
            available = [unit_id for unit_id in candidates if self.pool[unit_id] > 0]
            if not available:
                break
            unit_id = self.rng.choice(available)
            self.pool[unit_id] -= 1
            self.roster.add(unit_id)
            result.append(UnitInstance(unit_id, 1))
        return tuple(result)
