"""运营模拟使用的数据类型。"""

from dataclasses import dataclass, field
from enum import StrEnum


class NodeType(StrEnum):
    """一个节点的局外流程类型。"""

    BATTLE = "battle"
    REWARD = "reward"
    ENCOUNTER = "encounter"
    BOSS = "boss"


class Position(StrEnum):
    """货币战争角色站位。"""

    FRONT = "front"
    BACK = "back"
    FLEX = "flex"


class InvestmentRarity(StrEnum):
    """投资策略品质。"""

    SILVER = "silver"
    GOLD = "gold"
    PRISMATIC = "prismatic"


@dataclass(frozen=True)
class UnitDefinition:
    """可在商店出现的角色。"""

    unit_id: str
    name: str
    cost: int
    position: Position
    tags: tuple[str, ...]
    traits: tuple[str, ...]


@dataclass(frozen=True)
class UnitInstance:
    """场上或备战席中的角色实例。"""

    unit_id: str
    star: int = 1


@dataclass(frozen=True)
class ShopOffer:
    """商店中的一个位置。None 表示牌池无法补足该位置。"""

    unit_id: str | None


@dataclass(frozen=True)
class AffixEffect:
    """敌人词缀转换为运营模型的数值影响。

    战斗内部机制由外部战斗模型处理。本模型只保留会影响运营选择或
    抽象战斗胜率的修正。
    """

    initial_health_delta: int = 0
    enemy_rating_multiplier: float = 1.0
    plane_enemy_rating_multipliers: tuple[float, float, float] = (1.0, 1.0, 1.0)
    team_power_multiplier: float = 1.0
    cost_power_multipliers: tuple[float, float, float, float, float] = (
        1.0,
        1.0,
        1.0,
        1.0,
        1.0,
    )
    star_power_multipliers: tuple[float, float, float] = (1.0, 1.0, 1.0)
    xp_extra_cost_after_level: int = 0
    xp_extra_cost: int = 0
    reward_multiplier: float = 1.0


@dataclass(frozen=True)
class Affix:
    """一个开局敌人词缀。"""

    affix_id: str
    name: str
    description: str
    effect: AffixEffect = field(default_factory=AffixEffect)


@dataclass(frozen=True)
class DifficultyPreset:
    """可自选的模拟难度。"""

    difficulty_id: str
    name: str
    base_enemy_rating: float
    rating_per_node: float
    affix_count: int


@dataclass(frozen=True)
class InvestmentEffect:
    """当前模型支持的投资效果。"""

    gold: int = 0
    xp: int = 0
    free_rerolls: int = 0
    node_free_rerolls: int = 0
    plane_free_rerolls: int = 0
    plane_xp: int = 0
    interest_cap: int | None = None
    idle_node_free_rerolls: int = 0
    idle_node_gold_from_interest: bool = False
    node_gold: int = 0
    node_gold_steps: int = 0
    boss_gold: int = 0
    last_supply_gold: int = 0
    set_streak: int | None = None
    level_goal: int | None = None
    level_goal_gold: int = 0
    lock_purchase_nodes: int = 0
    deferred_gold: int = 0
    free_reroll_chance: float = 0.0
    xp_cost_delta: int = 0
    team_power_multiplier: float = 1.0
    trait_power_multiplier: float = 1.0
    enemy_rating_delta: int = 0
    swap_health_gold: bool = False


@dataclass(frozen=True)
class Investment:
    """一个投资策略。"""

    investment_id: str
    name: str
    rarity: InvestmentRarity
    planes: tuple[int, ...]
    description: str
    effect: InvestmentEffect = field(default_factory=InvestmentEffect)


@dataclass(frozen=True)
class EnvironmentEffect:
    """开局投资环境转换为模型参数。"""

    initial_gold: int = 0
    starting_unit_cost: int | None = None
    starting_unit_count: int = 0
    plane_gold: tuple[int, int, int] = (0, 0, 0)
    strategy_rarity_mode: InvestmentRarity | None = None
    first_strategy_rarity: InvestmentRarity | None = None
    reward_multiplier: float = 1.0
    remove_random_affix: bool = False
    team_power_per_node: float = 0.0


@dataclass(frozen=True)
class InvestmentEnvironment:
    """开局选择的投资环境。"""

    environment_id: str
    name: str
    description: str
    effect: EnvironmentEffect = field(default_factory=EnvironmentEffect)


@dataclass(frozen=True)
class GameRules:
    """集中维护的运营规则。"""

    schema_version: int
    game_version: str
    starting_health: int
    starting_gold: int
    starting_level: int
    starting_supply: int
    shop_size: int
    refresh_cost: int
    xp_purchase_cost: int
    xp_purchase_amount: int
    interest_step_gold: int
    interest_cap: int
    base_battle_gold: int
    battle_temperature: float
    shop_odds: tuple[tuple[float, float, float, float, float], ...]
    xp_to_next_level: tuple[int, ...]
    card_copies_by_cost: tuple[int, int, int, int, int]
    node_types: tuple[NodeType, ...]
    investment_nodes: tuple[int, ...]
    strategy_rarity_weights: tuple[float, float, float]
    strategy_enemy_rating_delta: tuple[int, int, int]
    loss_damage: tuple[int, int, int, int]


@dataclass(frozen=True)
class RuleMetadata:
    """规则包来源与验证状态。"""

    public_source: str
    public_revision: str
    retrieved_on: str
    local_client_build: str
    validation_status: str
    assumptions: tuple[str, ...]


@dataclass(frozen=True)
class RulePack:
    """一次模拟使用的全部静态规则。"""

    rules: GameRules
    metadata: RuleMetadata
    units: dict[str, UnitDefinition]
    difficulties: dict[str, DifficultyPreset]
    affixes: dict[str, Affix]
    investments: dict[str, Investment]
    environments: dict[str, InvestmentEnvironment]


@dataclass(frozen=True)
class NodeDefinition:
    """节点模板中的一项。"""

    index: int
    plane: int
    node_type: NodeType


@dataclass(frozen=True)
class NodeResult:
    """一次节点推进的结果，供回放和统计。"""

    node_index: int
    node_type: NodeType
    won: bool
    win_probability: float
    team_power: float
    enemy_rating: float
    health_delta: int
    gold_delta: int
    streak_after: int


@dataclass(frozen=True)
class RunObservation:
    """策略可见信息，不包含商店牌池等隐藏状态。"""

    node_index: int
    node_type: NodeType
    plane: int
    health: int
    max_health: int
    gold: int
    level: int
    xp: int
    xp_to_next_level: int | None
    streak: int
    shop_offers: tuple[ShopOffer, ...]
    shop_locked: bool
    free_rerolls: int
    active_units: tuple[UnitInstance, ...]
    roster: tuple[UnitInstance, ...]
    team_power: float
    win_probability: float
    selected_investments: tuple[str, ...]
    investment_offers: tuple[str, ...]
    affixes: tuple[str, ...]
    ended: bool
    won: bool
