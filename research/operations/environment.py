"""不含战斗过程的整局运营环境。"""

import random
from collections.abc import Iterable
from dataclasses import dataclass, field

from .battle_progress import (
    BattleContext,
    BattleProgressProvider,
    FixedBattleProgressProvider,
)
from .investments import InvestmentActionError, InvestmentEngine
from .market import Market
from .models import (
    Affix,
    Investment,
    InvestmentRarity,
    NodeDefinition,
    NodeResult,
    NodeType,
    Position,
    RulePack,
    RunObservation,
    UnitInstance,
)


class OperationActionError(ValueError):
    """当前运营阶段不接受该操作。"""


@dataclass(frozen=True)
class PendingGold:
    """在指定节点开始时发放的金币。"""

    node_index: int
    amount: int
    reason: str


@dataclass
class OperationEnvironment:
    """推进市场、投资、难度词缀和抽象战斗结果。

    完整牌池和未来随机结果保存在环境内部；observe() 只返回玩家当前可见
    的状态。所有随机过程共享显式传入的随机源。
    """

    pack: RulePack
    rng: random.Random
    difficulty_id: str
    environment_id: str | None = None
    affix_ids: tuple[str, ...] | None = None
    enemy_difficulty: int | None = None
    affix_count: int | None = None
    battle_progress: BattleProgressProvider = field(default_factory=FixedBattleProgressProvider)
    market: Market = field(init=False)
    investments: InvestmentEngine = field(init=False)
    health: int = field(init=False)
    max_health: int = field(init=False)
    gold: int = field(init=False)
    node_index: int = field(init=False, default=0)
    plane: int = field(init=False, default=0)
    streak: int = field(init=False, default=0)
    affixes: tuple[Affix, ...] = field(init=False)
    interest_cap: int = field(init=False)
    xp_cost_adjustment: int = field(init=False, default=0)
    enemy_rating_adjustment: int = field(init=False, default=0)
    investment_power_multiplier: float = field(init=False, default=1.0)
    investment_trait_multiplier: float = field(init=False, default=1.0)
    environment_power_bonus: float = field(init=False, default=0.0)
    spent_last_node: bool = field(init=False, default=False)
    spent_this_node: bool = field(init=False, default=False)
    paid_level_goals: set[int] = field(init=False, default_factory=set)
    pending_gold: list[PendingGold] = field(init=False, default_factory=list)
    results: list[NodeResult] = field(init=False, default_factory=list)
    ended: bool = field(init=False, default=False)
    won: bool = field(init=False, default=False)
    last_battle_progress: int | None = field(init=False, default=None)
    last_battle_provider: str | None = field(init=False, default=None)
    front_slots: list[UnitInstance | None] = field(init=False, default_factory=list)
    back_slots: list[UnitInstance | None] = field(init=False, default_factory=list)
    bench_slots: list[UnitInstance | None] = field(init=False, default_factory=list)

    def __post_init__(self) -> None:
        difficulty = self.pack.difficulties.get(self.difficulty_id)
        if difficulty is None and (self.enemy_difficulty is None or self.affix_count is None):
            raise ValueError(f"未知难度：{self.difficulty_id}")
        if self.enemy_difficulty is None:
            self.enemy_difficulty = difficulty.enemy_difficulty
        if self.affix_count is None:
            self.affix_count = difficulty.affix_count
        if not 1 <= self.enemy_difficulty <= self.pack.rules.enemy_difficulty_max:
            raise ValueError("敌人难度超出允许范围")
        if (
            not 0
            <= self.affix_count
            <= min(
                self.pack.rules.affix_count_max,
                len(self.pack.affixes),
            )
        ):
            raise ValueError("词缀数量超出允许范围")
        if self.environment_id is not None and self.environment_id not in self.pack.environments:
            raise ValueError(f"未知投资环境：{self.environment_id}")

        if self.affix_ids is None:
            selected_ids = tuple(self.rng.sample(sorted(self.pack.affixes), self.affix_count))
        else:
            unknown = set(self.affix_ids) - set(self.pack.affixes)
            if unknown:
                raise ValueError(f"未知词缀：{sorted(unknown)}")
            if len(set(self.affix_ids)) != len(self.affix_ids):
                raise ValueError("词缀不能重复")
            selected_ids = self.affix_ids
        selected = [self.pack.affixes[affix_id] for affix_id in selected_ids]

        environment_id = self.environment_id
        if environment_id is None:
            environment_id = self.rng.choice(sorted(self.pack.environments))
            self.environment_id = environment_id
        environment = self.pack.environments[environment_id]
        if environment.effect.remove_random_affix and selected:
            selected.pop(self.rng.randrange(len(selected)))

        self.affixes = tuple(selected)
        self.max_health = self.pack.rules.starting_health
        initial_health_delta = sum(affix.effect.initial_health_delta for affix in self.affixes)
        self.health = max(1, self.max_health + initial_health_delta)
        self.gold = self.pack.rules.starting_gold + environment.effect.initial_gold
        self.interest_cap = self.pack.rules.interest_cap

        self.market = Market(self.pack.rules, self.pack.units, self.rng)
        self.investments = InvestmentEngine(self.pack, self.rng, environment.effect)
        self.front_slots = [None] * self.pack.rules.front_capacity
        self.back_slots = [None] * self.pack.rules.back_capacity
        self.bench_slots = [None] * self.pack.rules.bench_capacity

        if environment.effect.starting_unit_count:
            starting_cost = environment.effect.starting_unit_cost
            if starting_cost is not None:
                starting_cost = min(
                    starting_cost,
                    self.pack.rules.starting_unit_max_cost,
                )
            self.market.take_random_units(
                environment.effect.starting_unit_count,
                cost=starting_cost,
                max_cost=self.pack.rules.starting_unit_max_cost,
            )
        else:
            self.market.take_random_units(
                self.pack.rules.starting_supply,
                max_cost=self.pack.rules.starting_unit_max_cost,
            )
        self._reconcile_slots(add_missing_to_bench=True)
        self._auto_deploy()
        self._enter_node()

    @property
    def current_node(self) -> NodeDefinition:
        """当前节点。"""

        if self.node_index >= len(self.pack.rules.node_types):
            raise OperationActionError("对局已经结束")
        node_type = self.pack.rules.node_types[self.node_index]
        return NodeDefinition(
            self.node_index,
            self.pack.rules.node_ids[self.node_index],
            self.pack.rules.node_planes[self.node_index],
            node_type,
        )

    @property
    def node_template(self) -> tuple[NodeDefinition, ...]:
        """返回整局节点模板，供网页显示路线。"""

        return tuple(
            NodeDefinition(
                index,
                self.pack.rules.node_ids[index],
                self.pack.rules.node_planes[index],
                node_type,
            )
            for index, node_type in enumerate(self.pack.rules.node_types)
        )

    def observe(self) -> RunObservation:
        """返回策略可见状态。"""

        active_units = self.active_units()
        team_power = self.team_power()
        node = self.current_node if not self.ended else None
        return RunObservation(
            node_index=self.node_index,
            node_type=NodeType.BOSS if node is None else node.node_type,
            plane=self.plane,
            health=self.health,
            max_health=self.max_health,
            gold=self.gold,
            level=self.market.level,
            xp=self.market.xp,
            xp_to_next_level=self.market.xp_to_next,
            streak=self.streak,
            shop_offers=self.market.offers,
            shop_locked=self.market.locked,
            free_rerolls=self.market.free_rerolls,
            active_units=active_units,
            roster=self.market.roster.instances(),
            team_power=team_power,
            last_battle_progress=self.last_battle_progress,
            last_battle_provider=self.last_battle_provider,
            selected_investments=tuple(
                investment.investment_id for investment in self.investments.selected
            ),
            investment_offers=tuple(
                investment.investment_id for investment in self.investments.current_offers
            ),
            affixes=tuple(affix.affix_id for affix in self.affixes),
            ended=self.ended,
            won=self.won,
        )

    def set_shop_locked(self, locked: bool) -> None:
        """设置是否保留商店。"""

        self._ensure_active()
        self.market.set_locked(locked)

    def reroll_shop(self) -> None:
        """刷新商店。"""

        self._ensure_market_phase()
        gold, paid = self.market.refresh_with_resources(self.gold)
        self.gold = gold
        self.spent_this_node = self.spent_this_node or paid

    def buy_shop_offer(self, offer_index: int) -> tuple[str, ...]:
        """购买一个商店角色，返回合成事件。"""

        self._ensure_market_phase()
        if offer_index < 0 or offer_index >= len(self.market.offers):
            raise OperationActionError("商店位置不存在")
        if self.bench_count >= self.pack.rules.bench_capacity:
            offer = self.market.offers[offer_index]
            if offer.unit_id is None or not self._purchase_would_merge(offer.unit_id):
                raise OperationActionError("备战席已满")
        gold, merges = self.market.buy_unit(offer_index, self.gold)
        self.gold = gold
        self.spent_this_node = True
        self._reconcile_slots(add_missing_to_bench=True)
        return merges

    def sell_unit(self, unit_id: str, star: int) -> None:
        """出售角色并把卡牌归还牌池。"""

        self._ensure_market_phase()
        self.gold = self.market.sell_unit(unit_id, star, self.gold)
        self._reconcile_slots(add_missing_to_bench=False)

    @property
    def bench_count(self) -> int:
        """当前备战席角色实例数量。"""

        return sum(unit is not None for unit in self.bench_slots)

    @property
    def active_count(self) -> int:
        return sum(unit is not None for unit in self.front_slots) + sum(
            unit is not None for unit in self.back_slots
        )

    def front_units(self) -> tuple[UnitInstance, ...]:
        return tuple(unit for unit in self.front_slots if unit is not None)

    def back_units(self) -> tuple[UnitInstance, ...]:
        return tuple(unit for unit in self.back_slots if unit is not None)

    def bench_units(self) -> tuple[UnitInstance, ...]:
        """返回未上阵角色实例。"""

        return tuple(unit for unit in self.bench_slots if unit is not None)

    def set_unit_position(
        self,
        unit_id: str,
        star: int,
        position: Position | None,
    ) -> None:
        """把一名角色移动到前台、后台或备战席。"""

        self._ensure_market_phase()
        unit = UnitInstance(unit_id, star)
        source = self._find_slot(unit)
        if source is None and self.market.roster.units.get((unit_id, star), 0) == 0:
            raise OperationActionError("没有可部署的指定角色")
        if source is None:
            self._reconcile_slots(add_missing_to_bench=True)
            source = self._find_slot(unit)
            if source is None:
                raise OperationActionError("角色不在可用格位")
        if position is not None and position not in (Position.FRONT, Position.BACK):
            raise OperationActionError("角色只能部署到前台或后台")
        if position is None:
            if source is None or source[0] == "bench":
                return
            if self.bench_count >= self.pack.rules.bench_capacity:
                raise OperationActionError("备战席已满")
            self.bench_slots[self._first_empty(self.bench_slots)] = unit
            self._set_slot(source, None)
            return
        target_location = "front" if position is Position.FRONT else "back"
        target = self._slots_for(target_location)
        target_index = self._first_empty(target)
        if target_index is None:
            raise OperationActionError(f"{'前台' if position is Position.FRONT else '后台'}已满")
        if source[0] == "bench":
            if self.active_count >= self.market.active_capacity:
                raise OperationActionError("上阵人数已达到商店等级上限")
            self._set_slot(source, None)
        else:
            self._set_slot(source, None)
        target[target_index] = unit

    def swap_unit_slots(
        self,
        source_location: str,
        source_index: int,
        target_location: str,
        target_index: int,
    ) -> None:
        """交换两个格位，并在任何约束失败时保持原布局。"""

        self._ensure_market_phase()
        source_slots = self._slots_for(source_location)
        target_slots = self._slots_for(target_location)
        if not 0 <= source_index < len(source_slots):
            raise OperationActionError("拖动起点不存在")
        if not 0 <= target_index < len(target_slots):
            raise OperationActionError("拖动终点不存在")
        if source_slots[source_index] is None:
            raise OperationActionError("拖动起点没有角色")
        if source_location == target_location and source_index == target_index:
            return
        source_unit = source_slots[source_index]
        target_unit = target_slots[target_index]
        next_active = self.active_count
        if source_location == "bench" and target_location != "bench" and target_unit is None:
            next_active += 1
        elif source_location != "bench" and target_location == "bench" and target_unit is None:
            next_active -= 1
        if next_active > self.market.active_capacity:
            raise OperationActionError("上阵人数已达到商店等级上限")
        before = (
            list(self.front_slots),
            list(self.back_slots),
            list(self.bench_slots),
        )
        source_slots[source_index] = target_unit
        target_slots[target_index] = source_unit
        if not self._slots_match_roster():
            self.front_slots, self.back_slots, self.bench_slots = before
            raise OperationActionError("交换后角色归属不合法")

    def buy_xp(self, purchase_count: int = 1) -> tuple[int, ...]:
        """购买经验，返回本次达到的等级。"""

        self._ensure_market_phase()
        extra_cost = sum(
            affix.effect.xp_extra_cost
            for affix in self.affixes
            if affix.effect.xp_extra_cost_after_level
            and self.market.level >= affix.effect.xp_extra_cost_after_level
        )
        gold, levels = self.market.buy_xp(
            self.gold,
            purchase_count=purchase_count,
            cost_adjustment=self.xp_cost_adjustment,
            extra_cost=extra_cost,
        )
        self.spent_this_node = self.spent_this_node or gold != self.gold
        self.gold = gold
        self._handle_levels(levels)
        return levels

    def choose_investment(self, offer_index: int) -> Investment:
        """选择当前投资策略并立即应用其运营效果。"""

        self._ensure_active()
        try:
            investment = self.investments.choose(offer_index)
        except InvestmentActionError as error:
            raise OperationActionError(str(error)) from error
        self._apply_investment(investment)
        return investment

    def advance_node(self) -> NodeResult:
        """结算当前节点并进入下一个节点。"""

        self._ensure_active()
        if self.investments.current_offers:
            raise OperationActionError("需要先选择一个投资策略")
        node = self.current_node
        team_power = self.team_power()
        enemy_rating = self.enemy_rating(node)
        battle = self.battle_progress.resolve(
            BattleContext(
                node_index=node.index,
                node_type=node.node_type,
                plane=node.plane,
                health=self.health,
                gold=self.gold,
                level=self.market.level,
                streak=self.streak,
                team_power=team_power,
                enemy_rating=enemy_rating,
                active_units=self.active_units(),
                difficulty_id=self.difficulty_id,
                affix_ids=tuple(affix.affix_id for affix in self.affixes),
                investment_ids=tuple(
                    investment.investment_id for investment in self.investments.selected
                ),
            )
        )
        won = battle.progress_percent == 100

        old_health = self.health
        old_gold = self.gold
        if won:
            self.streak += 1
        else:
            self.streak = 0

        interest = min(self.gold // self.pack.rules.interest_step_gold, self.interest_cap)
        streak_bonus = min(5, 1 + self.streak // 2) if won else 0
        base_gold = self.pack.rules.base_battle_gold
        if node.node_type is NodeType.REWARD:
            environment = self.pack.environments[self.environment_id]
            base_gold = round(base_gold * environment.effect.reward_multiplier)
        self.gold += base_gold + interest + streak_bonus

        if not won:
            damage_index = {
                NodeType.BATTLE: 0,
                NodeType.REWARD: 1,
                NodeType.ENCOUNTER: 2,
                NodeType.BOSS: 3,
            }[node.node_type]
            self.health = max(0, self.health - self.pack.rules.loss_damage[damage_index])

        result = NodeResult(
            node_index=self.node_index,
            node_type=node.node_type,
            won=won,
            battle_progress=battle.progress_percent,
            battle_provider=battle.provider_id,
            team_power=team_power,
            enemy_rating=enemy_rating,
            health_delta=self.health - old_health,
            gold_delta=self.gold - old_gold,
            streak_after=self.streak,
        )
        self.results.append(result)
        self.last_battle_progress = battle.progress_percent
        self.last_battle_provider = battle.provider_id

        is_final_node = self.node_index == len(self.pack.rules.node_types) - 1
        if self.health <= 0 or is_final_node:
            self.ended = True
            self.won = is_final_node and won and self.health > 0
            return result

        self.spent_last_node = self.spent_this_node
        self.spent_this_node = False
        self.node_index += 1
        self._enter_node()
        return result

    def active_units(self) -> tuple[UnitInstance, ...]:
        """返回玩家当前手动部署的前后台角色。"""

        return self.front_units() + self.back_units()

    def _auto_deploy(self) -> None:
        """开局时按角色站位自动部署默认阵容。"""

        for bench_index, unit in enumerate(self.bench_slots):
            if unit is None:
                continue
            if self.active_count >= self.market.active_capacity:
                break
            preferred = self.pack.units[unit.unit_id].position
            candidates = (
                (Position.FRONT, Position.BACK)
                if preferred is not Position.BACK
                else (Position.BACK, Position.FRONT)
            )
            for position in candidates:
                target = self.front_slots if position is Position.FRONT else self.back_slots
                target_index = self._first_empty(target)
                if target_index is not None:
                    target[target_index] = unit
                    self.bench_slots[bench_index] = None
                    break

    def _reconcile_slots(self, *, add_missing_to_bench: bool) -> None:
        """让格位中的角色与拥有列表一致，并保留现有格位顺序。"""

        current: dict[tuple[str, int], int] = {}
        for unit in (*self.front_slots, *self.back_slots, *self.bench_slots):
            if unit is not None:
                key = (unit.unit_id, unit.star)
                current[key] = current.get(key, 0) + 1
        for key, count in current.items():
            owned = self.market.roster.units.get(key, 0)
            while count > owned:
                if not self._remove_last_occurrence(key):
                    break
                count -= 1
        while self.active_count > self.market.active_capacity:
            self._move_last_active_to_bench()
        if add_missing_to_bench:
            placed = self._slot_counts()
            for key, owned in self.market.roster.units.items():
                for _ in range(owned - placed.get(key, 0)):
                    index = self._first_empty(self.bench_slots)
                    if index is None:
                        raise OperationActionError("备战席已满")
                    self.bench_slots[index] = UnitInstance(*key)
                    placed[key] = placed.get(key, 0) + 1

    def _remove_last_occurrence(self, key: tuple[str, int]) -> bool:
        for slots in (self.bench_slots, self.back_slots, self.front_slots):
            for index in range(len(slots) - 1, -1, -1):
                unit = slots[index]
                if unit is not None and (unit.unit_id, unit.star) == key:
                    slots[index] = None
                    return True
        return False

    def _move_last_active_to_bench(self) -> None:
        bench_index = self._first_empty(self.bench_slots)
        if bench_index is None:
            raise OperationActionError("备战席已满")
        for slots in (self.back_slots, self.front_slots):
            for index in range(len(slots) - 1, -1, -1):
                unit = slots[index]
                if unit is not None:
                    self.bench_slots[bench_index] = unit
                    slots[index] = None
                    return

    def _slot_counts(self) -> dict[tuple[str, int], int]:
        counts: dict[tuple[str, int], int] = {}
        for unit in (*self.front_slots, *self.back_slots, *self.bench_slots):
            if unit is not None:
                key = (unit.unit_id, unit.star)
                counts[key] = counts.get(key, 0) + 1
        return counts

    def _slots_match_roster(self) -> bool:
        return self._slot_counts() == {
            key: count for key, count in self.market.roster.units.items() if count
        }

    def _purchase_would_merge(self, unit_id: str) -> bool:
        counts = dict(self.market.roster.units)
        key = (unit_id, 1)
        while key[1] < 4:
            if counts.get(key, 0) >= 2:
                return True
            key = (unit_id, key[1] + 1)
        return False

    def _find_slot(self, unit: UnitInstance) -> tuple[str, int] | None:
        for location, slots in (
            ("front", self.front_slots),
            ("back", self.back_slots),
            ("bench", self.bench_slots),
        ):
            for index, candidate in enumerate(slots):
                if candidate == unit:
                    return location, index
        return None

    def _slots_for(self, location: str) -> list[UnitInstance | None]:
        if location == "front":
            return self.front_slots
        if location == "back":
            return self.back_slots
        if location == "bench":
            return self.bench_slots
        raise OperationActionError("未知阵容位置")

    @staticmethod
    def _first_empty(slots: list[UnitInstance | None]) -> int | None:
        for index, unit in enumerate(slots):
            if unit is None:
                return index
        return None

    def _set_slot(self, source: tuple[str, int], unit: UnitInstance | None) -> None:
        self._slots_for(source[0])[source[1]] = unit

    def team_power(self) -> float:
        """计算抽象阵容强度。"""

        active = self.active_units()
        unit_power = sum(self._unit_power(unit) for unit in active)
        trait_counts: dict[str, int] = {}
        for unit in active:
            for trait in self.pack.units[unit.unit_id].traits:
                trait_counts[trait] = trait_counts.get(trait, 0) + 1
        trait_power = sum(
            min(2.0, (count - 1) * 0.45) for count in trait_counts.values() if count >= 2
        )
        affix_team_multiplier = self._product(
            affix.effect.team_power_multiplier for affix in self.affixes
        )
        return (
            unit_power * affix_team_multiplier
            + trait_power * self.investment_trait_multiplier * affix_team_multiplier
            + self.environment_power_bonus
        ) * self.investment_power_multiplier

    def enemy_rating(self, node: NodeDefinition | None = None) -> float:
        """计算当前节点的抽象敌人强度。"""

        if node is None:
            node = self.current_node
        node_multiplier = {
            NodeType.BATTLE: 1.0,
            NodeType.REWARD: 0.8,
            NodeType.ENCOUNTER: 1.2,
            NodeType.BOSS: 1.5,
        }[node.node_type]
        affix_multiplier = self._product(
            affix.effect.enemy_rating_multiplier
            * affix.effect.plane_enemy_rating_multipliers[node.plane - 1]
            for affix in self.affixes
        )
        base = float(self.enemy_difficulty)
        return max(0.1, base * node_multiplier * affix_multiplier + self.enemy_rating_adjustment)

    def _unit_power(self, unit: UnitInstance) -> float:
        definition = self.pack.units.get(unit.unit_id)
        if definition is None:
            raise ValueError(f"未知角色：{unit.unit_id}")
        affix_cost = self._product(
            affix.effect.cost_power_multipliers[definition.cost - 1] for affix in self.affixes
        )
        affix_star = self._product(
            affix.effect.star_power_multipliers[min(unit.star, 3) - 1] for affix in self.affixes
        )
        return definition.cost * (1.0 + (unit.star - 1) * 1.5) * affix_cost * affix_star

    @staticmethod
    def _product(values: Iterable[float]) -> float:
        result = 1.0
        for value in values:
            result *= value
        return result

    def _apply_investment(self, investment: Investment) -> None:
        effect = investment.effect
        self.gold += effect.gold
        self.market.free_rerolls += effect.free_rerolls
        if effect.interest_cap is not None:
            self.interest_cap = max(self.interest_cap, effect.interest_cap)
        if effect.set_streak is not None:
            self.streak = effect.set_streak
        if effect.lock_purchase_nodes:
            self.market.purchase_lock_nodes = max(
                self.market.purchase_lock_nodes,
                effect.lock_purchase_nodes,
            )
        if effect.deferred_gold:
            self.pending_gold.append(
                PendingGold(
                    self.node_index + effect.lock_purchase_nodes,
                    effect.deferred_gold,
                    investment.name,
                )
            )
        if effect.node_gold:
            self.gold += effect.node_gold
            for offset in range(1, effect.node_gold_steps + 1):
                self.pending_gold.append(
                    PendingGold(
                        self.node_index + offset,
                        effect.node_gold,
                        investment.name,
                    )
                )
        self.market.free_reroll_chance += effect.free_reroll_chance
        self.xp_cost_adjustment += effect.xp_cost_delta
        self.investment_power_multiplier *= effect.team_power_multiplier
        self.investment_trait_multiplier *= effect.trait_power_multiplier
        rarity_index = (
            InvestmentRarity.SILVER,
            InvestmentRarity.GOLD,
            InvestmentRarity.PRISMATIC,
        ).index(investment.rarity)
        self.enemy_rating_adjustment += (
            self.pack.rules.strategy_enemy_rating_delta[rarity_index] + effect.enemy_rating_delta
        )
        if effect.level_goal is not None:
            if self.market.level >= effect.level_goal:
                self.gold += effect.level_goal_gold
                self.paid_level_goals.add(effect.level_goal)
        if effect.xp:
            levels = self.market.grant_xp(effect.xp)
            self._handle_levels(levels)
        if effect.swap_health_gold:
            previous_health, previous_gold = self.health, self.gold
            self.health = min(self.max_health, previous_gold)
            self.gold = previous_health

    def _handle_levels(self, levels: tuple[int, ...]) -> None:
        for level in levels:
            for investment in self.investments.selected:
                effect = investment.effect
                if effect.level_goal == level and level not in self.paid_level_goals:
                    self.gold += effect.level_goal_gold
                    self.paid_level_goals.add(level)

    def _enter_node(self) -> None:
        if self.node_index >= len(self.pack.rules.node_types):
            self.ended = True
            return
        node = self.current_node
        previous_plane = self.plane
        self.plane = node.plane
        environment = self.pack.environments[self.environment_id]
        if self.plane != previous_plane:
            self.gold += environment.effect.plane_gold[self.plane - 1]
            if self.plane > 1:
                for investment in self.investments.selected:
                    self.market.free_rerolls += investment.effect.plane_free_rerolls
                    levels = self.market.grant_xp(investment.effect.plane_xp)
                    self._handle_levels(levels)

        for investment in self.investments.selected:
            effect = investment.effect
            self.market.free_rerolls += effect.node_free_rerolls
            if effect.boss_gold and node.node_type is NodeType.BOSS:
                self.gold += effect.boss_gold
            if effect.last_supply_gold and self.node_index == 16:
                self.gold += effect.last_supply_gold

        if not self.spent_last_node:
            for investment in self.investments.selected:
                effect = investment.effect
                self.market.free_rerolls += effect.idle_node_free_rerolls
                if effect.idle_node_gold_from_interest:
                    self.gold += min(
                        self.gold // self.pack.rules.interest_step_gold,
                        self.interest_cap,
                    )

        due = [reward for reward in self.pending_gold if reward.node_index == self.node_index]
        for reward in due:
            self.gold += reward.amount
            self.pending_gold.remove(reward)

        self.environment_power_bonus += environment.effect.team_power_per_node
        self.market.start_node()
        if self.node_index in self.pack.rules.investment_nodes:
            self.investments.open_offers(plane=self.plane)

    def _ensure_active(self) -> None:
        if self.ended:
            raise OperationActionError("对局已经结束")

    def _ensure_market_phase(self) -> None:
        self._ensure_active()
        if self.investments.current_offers:
            raise OperationActionError("需要先选择一个投资策略")

    def _plane_for_node(self, index: int) -> int:
        return self.pack.rules.node_planes[index]
