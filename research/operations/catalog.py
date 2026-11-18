"""读取并验证运营模拟规则包。"""

import json
from pathlib import Path

from .models import (
    Affix,
    AffixEffect,
    DifficultyPreset,
    EnvironmentEffect,
    GameRules,
    Investment,
    InvestmentEffect,
    InvestmentEnvironment,
    InvestmentRarity,
    NodeType,
    Position,
    RuleMetadata,
    RulePack,
    UnitDefinition,
)

DEFAULT_PACK = Path(__file__).resolve().parents[2] / "core/operations/data/default_pack.json"
if not DEFAULT_PACK.is_file():
    DEFAULT_PACK = Path(__file__).with_name("data") / "default_pack.json"

DEFAULT_RULES = GameRules(
    schema_version=1,
    game_version="4.7.0",
    starting_health=100,
    starting_gold=10,
    starting_level=3,
    starting_supply=3,
    starting_unit_max_cost=3,
    enemy_difficulty_max=211,
    affix_count_max=53,
    shop_size=5,
    bench_capacity=9,
    front_capacity=4,
    back_capacity=6,
    refresh_cost=2,
    xp_purchase_cost=1,
    xp_purchase_amount=4,
    interest_step_gold=10,
    interest_cap=5,
    base_battle_gold=5,
    shop_odds=(
        (1.0, 0.0, 0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0, 0.0, 0.0),
        (0.65, 0.25, 0.10, 0.0, 0.0),
        (0.45, 0.33, 0.20, 0.02, 0.0),
        (0.30, 0.40, 0.25, 0.05, 0.0),
        (0.19, 0.30, 0.40, 0.10, 0.01),
        (0.18, 0.25, 0.32, 0.22, 0.03),
        (0.15, 0.20, 0.25, 0.30, 0.10),
        (0.05, 0.10, 0.20, 0.40, 0.25),
    ),
    xp_to_next_level=(0, 0, 4, 6, 20, 40, 52, 72, 84),
    card_copies_by_cost=(30, 25, 18, 10, 9),
    node_types=(
        NodeType.REWARD,
        NodeType.REWARD,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.ENCOUNTER,
        NodeType.REWARD,
        NodeType.BOSS,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.ENCOUNTER,
        NodeType.REWARD,
        NodeType.BOSS,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.BATTLE,
        NodeType.ENCOUNTER,
        NodeType.REWARD,
        NodeType.BOSS,
    ),
    node_ids=(
        "1-1",
        "1-2",
        "1-3",
        "1-4",
        "1-6",
        "1-7",
        "1-8",
        "1-9",
        "2-1",
        "2-2",
        "2-4",
        "2-5",
        "2-6",
        "2-7",
        "3-1",
        "3-2",
        "3-4",
        "3-5",
        "3-6",
        "3-7",
    ),
    node_planes=(1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3),
    investment_nodes=(2, 7, 13),
    strategy_rarity_weights=(0.68, 0.27, 0.05),
    strategy_enemy_rating_delta=(0, 3, 7),
    loss_damage=(8, 4, 10, 15),
)


def _as_object(value: object, label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} 必须是对象")
    for key, item in value.items():
        if not isinstance(key, str):
            raise ValueError(f"{label} 的字段名必须是字符串")
        if isinstance(item, dict | list):
            continue
    return value


def _as_list(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise ValueError(f"{label} 必须是数组")
    return value


def _as_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} 必须是非空字符串")
    return value


def _as_int(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{label} 必须是整数")
    return value


def _as_float(value: object, label: str) -> float:
    if not isinstance(value, int | float) or isinstance(value, bool):
        raise ValueError(f"{label} 必须是数值")
    return float(value)


def _as_bool(value: object, label: str) -> bool:
    if not isinstance(value, bool):
        raise ValueError(f"{label} 必须是布尔值")
    return value


def _string_tuple(value: object, label: str) -> tuple[str, ...]:
    return tuple(_as_string(item, f"{label}[]") for item in _as_list(value, label))


def _int_tuple(value: object, label: str, length: int | None = None) -> tuple[int, ...]:
    result = tuple(_as_int(item, f"{label}[]") for item in _as_list(value, label))
    if length is not None and len(result) != length:
        raise ValueError(f"{label} 需要包含 {length} 项")
    return result


def _float_tuple(value: object, label: str, length: int | None = None) -> tuple[float, ...]:
    result = tuple(_as_float(item, f"{label}[]") for item in _as_list(value, label))
    if length is not None and len(result) != length:
        raise ValueError(f"{label} 需要包含 {length} 项")
    return result


def _optional_int(data: dict[str, object], key: str) -> int | None:
    value = data.get(key)
    return None if value is None else _as_int(value, key)


def _parse_unit(value: object) -> UnitDefinition:
    data = _as_object(value, "unit")
    return UnitDefinition(
        unit_id=_as_string(data.get("id"), "unit.id"),
        name=_as_string(data.get("name"), "unit.name"),
        cost=_as_int(data.get("cost"), "unit.cost"),
        position=Position(_as_string(data.get("position"), "unit.position")),
        tags=_string_tuple(data.get("tags"), "unit.tags"),
        traits=_string_tuple(data.get("traits"), "unit.traits"),
    )


def _parse_affix_effect(value: object) -> AffixEffect:
    data = _as_object(value, "affix.effect")
    plane_multipliers = _float_tuple(
        data.get("plane_enemy_rating_multipliers", [1.0, 1.0, 1.0]),
        "affix.effect.plane_enemy_rating_multipliers",
        3,
    )
    cost_multipliers = _float_tuple(
        data.get("cost_power_multipliers", [1.0, 1.0, 1.0, 1.0, 1.0]),
        "affix.effect.cost_power_multipliers",
        5,
    )
    star_multipliers = _float_tuple(
        data.get("star_power_multipliers", [1.0, 1.0, 1.0]),
        "affix.effect.star_power_multipliers",
        3,
    )
    return AffixEffect(
        initial_health_delta=_as_int(
            data.get("initial_health_delta", 0), "affix.effect.initial_health_delta"
        ),
        enemy_rating_multiplier=_as_float(
            data.get("enemy_rating_multiplier", 1.0),
            "affix.effect.enemy_rating_multiplier",
        ),
        plane_enemy_rating_multipliers=(
            plane_multipliers[0],
            plane_multipliers[1],
            plane_multipliers[2],
        ),
        team_power_multiplier=_as_float(
            data.get("team_power_multiplier", 1.0),
            "affix.effect.team_power_multiplier",
        ),
        cost_power_multipliers=(
            cost_multipliers[0],
            cost_multipliers[1],
            cost_multipliers[2],
            cost_multipliers[3],
            cost_multipliers[4],
        ),
        star_power_multipliers=(
            star_multipliers[0],
            star_multipliers[1],
            star_multipliers[2],
        ),
        xp_extra_cost_after_level=_as_int(
            data.get("xp_extra_cost_after_level", 0),
            "affix.effect.xp_extra_cost_after_level",
        ),
        xp_extra_cost=_as_int(data.get("xp_extra_cost", 0), "affix.effect.xp_extra_cost"),
        reward_multiplier=_as_float(
            data.get("reward_multiplier", 1.0), "affix.effect.reward_multiplier"
        ),
    )


def _parse_affix(value: object) -> Affix:
    data = _as_object(value, "affix")
    return Affix(
        affix_id=_as_string(data.get("id"), "affix.id"),
        name=_as_string(data.get("name"), "affix.name"),
        description=_as_string(data.get("description"), "affix.description"),
        effect=_parse_affix_effect(data.get("effect", {})),
    )


def _parse_investment_effect(value: object) -> InvestmentEffect:
    data = _as_object(value, "investment.effect")
    return InvestmentEffect(
        gold=_as_int(data.get("gold", 0), "investment.effect.gold"),
        xp=_as_int(data.get("xp", 0), "investment.effect.xp"),
        free_rerolls=_as_int(data.get("free_rerolls", 0), "investment.effect.free_rerolls"),
        node_free_rerolls=_as_int(
            data.get("node_free_rerolls", 0),
            "investment.effect.node_free_rerolls",
        ),
        plane_free_rerolls=_as_int(
            data.get("plane_free_rerolls", 0),
            "investment.effect.plane_free_rerolls",
        ),
        plane_xp=_as_int(data.get("plane_xp", 0), "investment.effect.plane_xp"),
        interest_cap=_optional_int(data, "interest_cap"),
        idle_node_free_rerolls=_as_int(
            data.get("idle_node_free_rerolls", 0),
            "investment.effect.idle_node_free_rerolls",
        ),
        idle_node_gold_from_interest=_as_bool(
            data.get("idle_node_gold_from_interest", False),
            "investment.effect.idle_node_gold_from_interest",
        ),
        node_gold=_as_int(data.get("node_gold", 0), "investment.effect.node_gold"),
        node_gold_steps=_as_int(
            data.get("node_gold_steps", 0), "investment.effect.node_gold_steps"
        ),
        boss_gold=_as_int(data.get("boss_gold", 0), "investment.effect.boss_gold"),
        last_supply_gold=_as_int(
            data.get("last_supply_gold", 0), "investment.effect.last_supply_gold"
        ),
        set_streak=_optional_int(data, "set_streak"),
        level_goal=_optional_int(data, "level_goal"),
        level_goal_gold=_as_int(
            data.get("level_goal_gold", 0), "investment.effect.level_goal_gold"
        ),
        lock_purchase_nodes=_as_int(
            data.get("lock_purchase_nodes", 0),
            "investment.effect.lock_purchase_nodes",
        ),
        deferred_gold=_as_int(data.get("deferred_gold", 0), "investment.effect.deferred_gold"),
        free_reroll_chance=_as_float(
            data.get("free_reroll_chance", 0.0),
            "investment.effect.free_reroll_chance",
        ),
        xp_cost_delta=_as_int(data.get("xp_cost_delta", 0), "investment.effect.xp_cost_delta"),
        team_power_multiplier=_as_float(
            data.get("team_power_multiplier", 1.0),
            "investment.effect.team_power_multiplier",
        ),
        trait_power_multiplier=_as_float(
            data.get("trait_power_multiplier", 1.0),
            "investment.effect.trait_power_multiplier",
        ),
        enemy_rating_delta=_as_int(
            data.get("enemy_rating_delta", 0), "investment.effect.enemy_rating_delta"
        ),
        swap_health_gold=_as_bool(
            data.get("swap_health_gold", False), "investment.effect.swap_health_gold"
        ),
    )


def _parse_investment(value: object) -> Investment:
    data = _as_object(value, "investment")
    planes = _int_tuple(data.get("planes"), "investment.planes")
    if not planes or any(plane not in (1, 2, 3) for plane in planes):
        raise ValueError("investment.planes 只能包含 1、2、3")
    return Investment(
        investment_id=_as_string(data.get("id"), "investment.id"),
        name=_as_string(data.get("name"), "investment.name"),
        rarity=InvestmentRarity(_as_string(data.get("rarity"), "investment.rarity")),
        planes=planes,
        description=_as_string(data.get("description"), "investment.description"),
        effect=_parse_investment_effect(data.get("effect", {})),
    )


def _parse_environment_effect(value: object) -> EnvironmentEffect:
    data = _as_object(value, "environment.effect")
    plane_gold = _int_tuple(data.get("plane_gold", [0, 0, 0]), "environment.effect.plane_gold", 3)
    rarity_mode = data.get("strategy_rarity_mode")
    first_rarity = data.get("first_strategy_rarity")
    starting_cost = data.get("starting_unit_cost")
    return EnvironmentEffect(
        initial_gold=_as_int(data.get("initial_gold", 0), "environment.effect.initial_gold"),
        starting_unit_cost=(
            None if starting_cost is None else _as_int(starting_cost, "starting_unit_cost")
        ),
        starting_unit_count=_as_int(
            data.get("starting_unit_count", 0), "environment.effect.starting_unit_count"
        ),
        plane_gold=(plane_gold[0], plane_gold[1], plane_gold[2]),
        strategy_rarity_mode=(
            None
            if rarity_mode is None
            else InvestmentRarity(_as_string(rarity_mode, "strategy_rarity_mode"))
        ),
        first_strategy_rarity=(
            None
            if first_rarity is None
            else InvestmentRarity(_as_string(first_rarity, "first_strategy_rarity"))
        ),
        reward_multiplier=_as_float(
            data.get("reward_multiplier", 1.0),
            "environment.effect.reward_multiplier",
        ),
        remove_random_affix=_as_bool(
            data.get("remove_random_affix", False),
            "environment.effect.remove_random_affix",
        ),
        team_power_per_node=_as_float(
            data.get("team_power_per_node", 0.0),
            "environment.effect.team_power_per_node",
        ),
    )


def _parse_environment(value: object) -> InvestmentEnvironment:
    data = _as_object(value, "environment")
    return InvestmentEnvironment(
        environment_id=_as_string(data.get("id"), "environment.id"),
        name=_as_string(data.get("name"), "environment.name"),
        description=_as_string(data.get("description"), "environment.description"),
        effect=_parse_environment_effect(data.get("effect", {})),
    )


def _parse_difficulty(value: object) -> DifficultyPreset:
    data = _as_object(value, "difficulty")
    affix_count = _as_int(data.get("affix_count"), "difficulty.affix_count")
    enemy_difficulty = _as_int(data.get("enemy_difficulty"), "difficulty.enemy_difficulty")
    if affix_count < 0 or enemy_difficulty < 1:
        raise ValueError("difficulty 数值非法")
    return DifficultyPreset(
        difficulty_id=_as_string(data.get("id"), "difficulty.id"),
        name=_as_string(data.get("name"), "difficulty.name"),
        enemy_difficulty=enemy_difficulty,
        affix_count=affix_count,
    )


def load_rule_pack(path: Path = DEFAULT_PACK) -> RulePack:
    """加载规则包并逐字段验证外部数据。"""

    raw = json.loads(path.read_text(encoding="utf-8"))
    root = _as_object(raw, "rule_pack")
    if _as_int(root.get("schema_version"), "schema_version") != DEFAULT_RULES.schema_version:
        raise ValueError("规则包版本与代码不兼容")
    if _as_string(root.get("game_version"), "game_version") != DEFAULT_RULES.game_version:
        raise ValueError("规则包游戏版本与规则常量不匹配")

    metadata_data = _as_object(root.get("metadata"), "metadata")
    metadata = RuleMetadata(
        public_source=_as_string(metadata_data.get("public_source"), "metadata.public_source"),
        public_revision=_as_string(
            metadata_data.get("public_revision"), "metadata.public_revision"
        ),
        retrieved_on=_as_string(metadata_data.get("retrieved_on"), "metadata.retrieved_on"),
        local_client_build=_as_string(
            metadata_data.get("local_client_build"), "metadata.local_client_build"
        ),
        validation_status=_as_string(
            metadata_data.get("validation_status"), "metadata.validation_status"
        ),
        assumptions=_string_tuple(metadata_data.get("assumptions"), "metadata.assumptions"),
        snapshot_id=(
            ""
            if metadata_data.get("snapshot_id") is None
            else _as_string(metadata_data.get("snapshot_id"), "metadata.snapshot_id")
        ),
        content_sha256=(
            ""
            if metadata_data.get("content_sha256") is None
            else _as_string(
                metadata_data.get("content_sha256"),
                "metadata.content_sha256",
            )
        ),
    )

    unit_items = list(map(_parse_unit, _as_list(root.get("units"), "units")))
    difficulty_items = list(
        map(_parse_difficulty, _as_list(root.get("difficulties"), "difficulties"))
    )
    affix_items = list(map(_parse_affix, _as_list(root.get("affixes"), "affixes")))
    investment_items = list(
        map(_parse_investment, _as_list(root.get("investments"), "investments"))
    )
    environment_items = list(
        map(_parse_environment, _as_list(root.get("environments"), "environments"))
    )
    for items, ids, label in (
        (unit_items, {item.unit_id for item in unit_items}, "units"),
        (
            difficulty_items,
            {item.difficulty_id for item in difficulty_items},
            "difficulties",
        ),
        (affix_items, {item.affix_id for item in affix_items}, "affixes"),
        (
            investment_items,
            {item.investment_id for item in investment_items},
            "investments",
        ),
        (
            environment_items,
            {item.environment_id for item in environment_items},
            "environments",
        ),
    ):
        if len(items) == 0 or len(items) != len(ids):
            raise ValueError(f"{label} 不能为空且 ID 不能重复")

    units = {item.unit_id: item for item in unit_items}
    difficulties = {item.difficulty_id: item for item in difficulty_items}
    affixes = {item.affix_id: item for item in affix_items}
    investments = {item.investment_id: item for item in investment_items}
    environments = {item.environment_id: item for item in environment_items}
    if any(unit.cost not in (1, 2, 3, 4, 5) for unit in units.values()):
        raise ValueError("角色费用必须位于 1 到 5")

    return RulePack(
        rules=DEFAULT_RULES,
        metadata=metadata,
        units=units,
        difficulties=difficulties,
        affixes=affixes,
        investments=investments,
        environments=environments,
        traits={},
    )
