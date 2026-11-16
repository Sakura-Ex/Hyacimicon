"""用于验证环境的基础运营策略。"""

import random
from dataclasses import dataclass

from .battle_progress import FixedBattleProgressProvider
from .environment import OperationActionError, OperationEnvironment
from .market import MarketActionError
from .models import Investment, Position, RulePack


@dataclass(frozen=True)
class SimulationSummary:
    """一批模拟的汇总结果。"""

    runs: int
    wins: int
    win_rate: float
    average_nodes: float
    average_health: float
    average_gold: float
    average_level: float


def investment_score(investment: Investment, *, health: int, gold: int) -> float:
    """给候选策略计算简单的运营收益估计。"""

    effect = investment.effect
    score = effect.gold + effect.xp * 0.5 + effect.free_rerolls * 0.25
    score += effect.node_gold * max(effect.node_gold_steps, 1) * 0.5
    score += (effect.team_power_multiplier - 1.0) * 40
    score += (effect.trait_power_multiplier - 1.0) * 20
    if effect.interest_cap is not None:
        score += effect.interest_cap * 1.5
    if effect.set_streak is not None:
        score += effect.set_streak
    if effect.level_goal_gold:
        score += effect.level_goal_gold * 0.75
    if effect.swap_health_gold:
        score += (gold - health) * 0.25
    if effect.lock_purchase_nodes:
        score -= effect.lock_purchase_nodes * 2
    return score


def run_greedy(
    pack: RulePack,
    *,
    difficulty_id: str,
    environment_id: str | None,
    seed: int,
    affix_ids: tuple[str, ...] | None = None,
    battle_progress: int = 100,
    enemy_difficulty: int | None = None,
    affix_count: int | None = None,
) -> OperationEnvironment:
    """运行一局贪心运营策略，返回完整可回放环境。"""

    rng = random.Random(seed)
    env = OperationEnvironment(
        pack=pack,
        rng=rng,
        difficulty_id=difficulty_id,
        environment_id=environment_id,
        affix_ids=affix_ids,
        battle_progress=FixedBattleProgressProvider(battle_progress),
        enemy_difficulty=enemy_difficulty,
        affix_count=affix_count,
    )
    while not env.ended:
        _choose_investment(env)
        _operate_market(env)
        env.advance_node()
    return env


def run_batch(
    pack: RulePack,
    *,
    difficulty_id: str,
    environment_id: str | None,
    runs: int,
    seed: int,
    battle_progress: int = 100,
    enemy_difficulty: int | None = None,
    affix_count: int | None = None,
) -> SimulationSummary:
    """用连续随机种子评估贪心策略。"""

    if runs < 1:
        raise ValueError("模拟局数必须大于 0")
    environments = [
        run_greedy(
            pack,
            difficulty_id=difficulty_id,
            environment_id=environment_id,
            seed=seed + offset,
            battle_progress=battle_progress,
            enemy_difficulty=enemy_difficulty,
            affix_count=affix_count,
        )
        for offset in range(runs)
    ]
    wins = sum(environment.won for environment in environments)
    return SimulationSummary(
        runs=runs,
        wins=wins,
        win_rate=wins / runs,
        average_nodes=sum(len(environment.results) for environment in environments) / runs,
        average_health=sum(environment.health for environment in environments) / runs,
        average_gold=sum(environment.gold for environment in environments) / runs,
        average_level=sum(environment.market.level for environment in environments) / runs,
    )


def _choose_investment(env: OperationEnvironment) -> None:
    offers = env.investments.current_offers
    if not offers:
        return
    scores = [
        investment_score(investment, health=env.health, gold=env.gold) for investment in offers
    ]
    env.choose_investment(max(range(len(offers)), key=scores.__getitem__))


def _operate_market(env: OperationEnvironment) -> None:
    for _ in range(20):
        visible = env.observe()
        affordable = []
        for index, offer in enumerate(visible.shop_offers):
            if offer.unit_id is None:
                continue
            unit = env.pack.units[offer.unit_id]
            if unit.cost <= env.gold:
                affordable.append((unit.cost, index))
        if affordable:
            _, index = max(affordable)
            offer = env.observe().shop_offers[index]
            unit_id = offer.unit_id
            try:
                env.buy_shop_offer(index)
            except OperationActionError:
                return
            if unit_id is not None and (unit_id, 1) in env.market.roster.units:
                preferred = env.pack.units[unit_id].position
                if preferred is Position.BACK:
                    positions = (Position.BACK,)
                elif preferred is Position.FRONT:
                    positions = (Position.FRONT,)
                else:
                    positions = (Position.FRONT, Position.BACK)
                for position in positions:
                    try:
                        env.set_unit_position(unit_id, 1, position)
                        break
                    except OperationActionError:
                        continue
            continue

        if env.market.level < 8 and env.gold >= 14:
            env.buy_xp()
            continue

        if env.market.free_rerolls > 0:
            try:
                env.reroll_shop()
            except MarketActionError:
                return
            continue
        return
