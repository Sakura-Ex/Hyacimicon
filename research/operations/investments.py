"""投资策略候选生成和选择。"""

import random
from dataclasses import dataclass, field

from .models import (
    EnvironmentEffect,
    Investment,
    InvestmentRarity,
    RulePack,
)


class InvestmentActionError(ValueError):
    """投资策略操作不符合当前阶段。"""


@dataclass
class InvestmentEngine:
    """维护当前三选一和玩家已选择的策略。"""

    pack: RulePack
    rng: random.Random
    environment_effect: EnvironmentEffect
    selected: list[Investment] = field(default_factory=list)
    current_offers: tuple[Investment, ...] = ()
    _offer_sequence: int = 0

    def open_offers(self, *, plane: int) -> None:
        """为指定位面生成三个不重复的策略。"""

        selected_ids = self._selected_ids()
        forced = self.environment_effect.strategy_rarity_mode
        same_rarity = [
            item
            for item in self.pack.investments.values()
            if forced is None or item.rarity is forced
        ]
        source = [item for item in same_rarity if item.investment_id not in selected_ids]
        if forced is not None and len(source) < 3:
            source = same_rarity
        plane_candidates = [item for item in source if forced is not None or plane in item.planes]
        eligible = plane_candidates if len(plane_candidates) >= 3 else source
        if len(source) < 3 or len(eligible) < 3:
            raise ValueError(f"第 {plane} 位面的可用投资策略不足三个")

        offers: list[Investment] = []
        for _ in range(3):
            rarity = self._draw_rarity()
            candidates = [item for item in source if item.rarity == rarity and item not in offers]
            if not candidates:
                candidates = [item for item in eligible if item not in offers]
            if not candidates:
                break
            offers.append(self.rng.choice(candidates))
        self.current_offers = tuple(offers)
        if len(self.current_offers) != 3:
            raise ValueError("无法生成三个不重复的投资策略")
        self._offer_sequence += 1

    def choose(self, index: int) -> Investment:
        """选择一个当前投资策略。"""

        if not self.current_offers:
            raise InvestmentActionError("当前没有可选择的投资策略")
        if index < 0 or index >= len(self.current_offers):
            raise InvestmentActionError("投资策略选项不存在")
        investment = self.current_offers[index]
        self.selected.append(investment)
        self.current_offers = ()
        return investment

    def _draw_rarity(self) -> InvestmentRarity:
        forced = self.environment_effect.strategy_rarity_mode
        if forced is not None:
            return forced
        if self._offer_sequence == 0 and self.environment_effect.first_strategy_rarity is not None:
            return self.environment_effect.first_strategy_rarity
        return self.rng.choices(
            (
                InvestmentRarity.SILVER,
                InvestmentRarity.GOLD,
                InvestmentRarity.PRISMATIC,
            ),
            weights=self.pack.rules.strategy_rarity_weights,
            k=1,
        )[0]

    def _selected_ids(self) -> set[str]:
        return {item.investment_id for item in self.selected}
