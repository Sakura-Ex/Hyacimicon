"""供网页和自动化程序共用的运营对局会话。"""

import random
import threading
import uuid
from dataclasses import dataclass, field
from pathlib import Path

from .battle_progress import FixedBattleProgressProvider
from .catalog import RulePack, load_rule_pack
from .environment import OperationEnvironment
from .models import Affix, Investment, NodeResult, Position, RunObservation, UnitInstance


class SessionNotFoundError(LookupError):
    """请求的对局会话不存在。"""


class SessionInputError(ValueError):
    """网页请求中的字段不符合接口约定。"""


@dataclass(frozen=True)
class RunEvent:
    """供网页监看的一条动作记录。"""

    sequence: int
    kind: str
    message: str


@dataclass
class OperationSession:
    """保存一局运营状态、战斗进度和动作记录。"""

    session_id: str
    pack: RulePack
    seed: int
    battle_progress: FixedBattleProgressProvider
    environment: OperationEnvironment
    events: list[RunEvent] = field(default_factory=list)

    @classmethod
    def create(
        cls,
        pack: RulePack,
        *,
        difficulty_id: str,
        environment_id: str | None,
        seed: int,
        affix_ids: tuple[str, ...] | None,
        battle_progress: int,
        enemy_difficulty: int | None = None,
        affix_count: int | None = None,
    ) -> "OperationSession":
        """创建一局并记录初始状态。"""

        provider = FixedBattleProgressProvider(battle_progress)
        environment = OperationEnvironment(
            pack=pack,
            rng=random.Random(seed),
            difficulty_id=difficulty_id,
            environment_id=environment_id,
            affix_ids=affix_ids,
            enemy_difficulty=enemy_difficulty,
            affix_count=affix_count,
            battle_progress=provider,
        )
        session = cls(
            session_id=uuid.uuid4().hex,
            pack=pack,
            seed=seed,
            battle_progress=provider,
            environment=environment,
        )
        session._append_event(
            "start",
            (
                f"创建对局：敌人难度 {environment.enemy_difficulty}，"
                f"词缀 {environment.affix_count}，"
                f"环境 {pack.environments[environment.environment_id].name}，"
                f"种子 {seed}，战斗进度 {battle_progress}%"
            ),
        )
        return session

    def apply(self, action: dict[str, object]) -> None:
        """执行一个经过字段验证的网页动作。"""

        action_type = _string(action, "type")
        if action_type == "set_battle_progress":
            self.battle_progress.set_progress(_integer(action, "progress"))
            self._append_event(
                "battle_progress",
                f"战斗进度设为 {self.battle_progress.progress_percent}%",
            )
            return
        if self.environment.ended:
            raise SessionInputError("对局已经结束")

        if action_type == "lock_shop":
            locked = _boolean(action, "locked")
            self.environment.set_shop_locked(locked)
            self._append_event("shop_lock", "锁定商店" if locked else "解除商店锁定")
            return
        if action_type == "reroll_shop":
            gold_before = self.environment.gold
            self.environment.reroll_shop()
            gold_used = gold_before - self.environment.gold
            suffix = "使用免费刷新" if gold_used == 0 else f"消耗 {gold_used} 金币"
            self._append_event("reroll", f"刷新商店，{suffix}")
            return
        if action_type == "buy_offer":
            index = _integer(action, "index")
            offers = self.environment.market.offers
            if index < 0 or index >= len(offers) or offers[index].unit_id is None:
                raise SessionInputError("商店位置没有可购买角色")
            unit_id = offers[index].unit_id
            unit = self.pack.units[unit_id]
            merges = self.environment.buy_shop_offer(index)
            message = f"购买 {unit.name}，消耗 {unit.cost} 金币"
            if merges:
                message += f"，触发合成 {', '.join(merges)}"
            self._append_event("purchase", message)
            return
        if action_type == "sell_unit":
            unit_id = _string(action, "unit_id")
            star = _integer(action, "star")
            unit = self.pack.units.get(unit_id)
            if unit is None:
                raise SessionInputError("未知角色")
            gold_before = self.environment.gold
            self.environment.sell_unit(unit_id, star)
            gold_gained = self.environment.gold - gold_before
            self._append_event(
                "sell",
                f"出售 {star} 星 {unit.name}，获得 {gold_gained} 金币",
            )
            return
        if action_type == "set_unit_position":
            unit_id = _string(action, "unit_id")
            star = _integer(action, "star")
            position_value = action.get("position")
            position = (
                None
                if position_value is None
                else Position(_required_string(position_value, "position"))
            )
            self.environment.set_unit_position(unit_id, star, position)
            labels = {
                None: "备战席",
                Position.FRONT: "前台",
                Position.BACK: "后台",
            }
            self._append_event(
                "deployment",
                f"将 {star} 星 {self.pack.units[unit_id].name} 移至{labels[position]}",
            )
            return
        if action_type == "swap_units":
            source_location, source_index = _slot_reference(action.get("source"))
            target_location, target_index = _slot_reference(action.get("target"))
            self.environment.swap_unit_slots(
                source_location,
                source_index,
                target_location,
                target_index,
            )
            source_unit = action.get("source")
            target_unit = action.get("target")
            if isinstance(source_unit, dict) and isinstance(target_unit, dict):
                source_location_text = _required_string(
                    source_unit.get("location"),
                    "source.location",
                )
                target_location_text = _required_string(
                    target_unit.get("location"),
                    "target.location",
                )
                self._append_event(
                    "deployment",
                    (
                        f"交换格位：{source_location_text}"
                        f" {_required_int(source_unit.get('index'), 'source.index') + 1}"
                        f" ↔ {target_location_text}"
                        f" {_required_int(target_unit.get('index'), 'target.index') + 1}"
                    ),
                )
            return
        if action_type == "buy_xp":
            count = _integer(action, "count", default=1)
            level_before = self.environment.market.level
            gold_before = self.environment.gold
            levels = self.environment.buy_xp(count)
            message = f"购买 {count} 次经验，消耗 {gold_before - self.environment.gold} 金币"
            if levels:
                message += f"，升至 {', '.join(str(level) for level in levels)} 级"
            else:
                message += f"，当前 {level_before} 级"
            self._append_event("xp", message)
            return
        if action_type == "choose_investment":
            index = _integer(action, "index")
            offers = self.environment.investments.current_offers
            if index < 0 or index >= len(offers):
                raise SessionInputError("投资策略位置不存在")
            investment = offers[index]
            self.environment.choose_investment(index)
            self._append_event(
                "investment",
                f"选择投资策略：{investment.rarity.value} {investment.name}",
            )
            return
        if action_type == "advance_node":
            result = self.environment.advance_node()
            self._record_node_result(result)
            return
        raise SessionInputError(f"未知操作：{action_type}")

    def state(self) -> dict[str, object]:
        """返回网页显示所需的运营状态与检查信息。"""

        observation = self.environment.observe()
        node_number = min(observation.node_index + 1, len(self.pack.rules.node_types))
        state: dict[str, object] = {
            "session_id": self.session_id,
            "seed": self.seed,
            "game_version": self.pack.rules.game_version,
            "snapshot_id": self.pack.metadata.snapshot_id,
            "phase": self._phase(observation),
            "finished": observation.ended,
            "won": observation.won,
            "node": {
                "number": node_number,
                "total": len(self.pack.rules.node_types),
                "index": observation.node_index,
                "id": self.environment.current_node.node_id
                if not observation.ended
                else self.pack.rules.node_ids[observation.node_index],
                "type": observation.node_type.value,
                "plane": observation.plane,
            },
            "node_template": [
                {
                    "index": node.index,
                    "id": node.node_id,
                    "type": node.node_type.value,
                    "plane": node.plane,
                }
                for node in self.environment.node_template
            ],
            "resources": {
                "health": observation.health,
                "max_health": observation.max_health,
                "gold": observation.gold,
                "level": observation.level,
                "xp": observation.xp,
                "xp_to_next_level": observation.xp_to_next_level,
                "streak": observation.streak,
                "free_rerolls": observation.free_rerolls,
            },
            "shop": [self._shop_offer_view(offer.unit_id) for offer in observation.shop_offers],
            "shop_locked": observation.shop_locked,
            "roster": [self._unit_view(unit) for unit in observation.roster],
            "active_units": [self._unit_view(unit) for unit in observation.active_units],
            "front_units": [self._unit_view(unit) for unit in self.environment.front_units()],
            "back_units": [self._unit_view(unit) for unit in self.environment.back_units()],
            "bench_units": [self._unit_view(unit) for unit in self.environment.bench_units()],
            "front_slots": self._slot_views(self.environment.front_slots),
            "back_slots": self._slot_views(self.environment.back_slots),
            "bench_slots": self._slot_views(self.environment.bench_slots),
            "team_limits": {
                "active": observation.level,
                "front": self.pack.rules.front_capacity,
                "back": self.pack.rules.back_capacity,
                "bench": self.pack.rules.bench_capacity,
                "bench_used": self.environment.bench_count,
            },
            "active_traits": self._active_trait_view(observation),
            "investments": {
                "selected": [
                    self._investment_view(self.pack.investments[investment_id])
                    for investment_id in observation.selected_investments
                ],
                "offers": [
                    self._investment_view(self.pack.investments[investment_id])
                    for investment_id in observation.investment_offers
                ],
            },
            "environment": self._environment_view(),
            "difficulty": self._difficulty_view(),
            "affixes": [
                self._affix_view(self.pack.affixes[affix_id]) for affix_id in observation.affixes
            ],
            "battle": {
                "provider_id": self.battle_progress.provider_id,
                "progress_percent": self.battle_progress.progress_percent,
                "last_progress": observation.last_battle_progress,
                "last_provider": observation.last_battle_provider,
            },
            "capabilities": self._capabilities(observation),
            "results": [self._node_result_view(result) for result in self.environment.results],
            "events": [
                {"sequence": event.sequence, "kind": event.kind, "message": event.message}
                for event in self.events[-100:]
            ],
            "debug": self._debug_view(observation),
        }
        return state

    def meta(self) -> dict[str, object]:
        """返回创建对局所需的选择项。"""

        return {
            "game_version": self.pack.rules.game_version,
            "snapshot_id": self.pack.metadata.snapshot_id,
            "content_sha256": self.pack.metadata.content_sha256,
            "validation": self.pack.metadata.validation_status,
            "character_count": len(self.pack.units),
            "trait_count": len(self.pack.traits),
            "assumptions": list(self.pack.metadata.assumptions),
            "difficulties": [
                {
                    "id": difficulty.difficulty_id,
                    "name": difficulty.name,
                    "enemy_difficulty": difficulty.enemy_difficulty,
                    "affix_count": difficulty.affix_count,
                }
                for difficulty in self.pack.difficulties.values()
            ],
            "difficulty_limits": {
                "enemy_min": 1,
                "enemy_max": self.pack.rules.enemy_difficulty_max,
                "affix_min": 0,
                "affix_max": min(
                    self.pack.rules.affix_count_max,
                    len(self.pack.affixes),
                ),
            },
            "environments": [
                {
                    "id": environment.environment_id,
                    "name": environment.name,
                    "description": environment.description,
                }
                for environment in self.pack.environments.values()
            ],
        }

    def _append_event(self, kind: str, message: str) -> None:
        self.events.append(RunEvent(len(self.events) + 1, kind, message))

    def _record_node_result(self, result: NodeResult) -> None:
        outcome = "胜利" if result.won else "失败"
        self._append_event(
            "node",
            (
                f"节点 {result.node_index + 1} {result.node_type.value} {outcome}，"
                f"战斗进度 {result.battle_progress}%，"
                f"生命 {result.health_delta:+d}，金币 {result.gold_delta:+d}"
            ),
        )
        if self.environment.ended:
            final = "整局胜利" if self.environment.won else "整局结束"
            self._append_event("finish", final)

    def _phase(self, observation: RunObservation) -> str:
        if observation.ended:
            return "finished"
        if observation.investment_offers:
            return "investment"
        return "operations"

    def _capabilities(self, observation: RunObservation) -> dict[str, bool]:
        preparation = not observation.ended and not observation.investment_offers
        return {
            "set_battle_progress": True,
            "lock_shop": preparation,
            "reroll_shop": preparation
            and (observation.free_rerolls > 0 or observation.gold >= self.pack.rules.refresh_cost),
            "buy_offer": preparation
            and any(offer.unit_id is not None for offer in observation.shop_offers),
            "sell_unit": preparation and bool(observation.roster),
            "set_unit_position": preparation and bool(observation.roster),
            "buy_xp": preparation and observation.level < 10,
            "choose_investment": bool(observation.investment_offers),
            "advance_node": preparation,
        }

    def _shop_offer_view(self, unit_id: str | None) -> dict[str, object]:
        if unit_id is None:
            return {"empty": True}
        unit = self.pack.units[unit_id]
        return {
            "empty": False,
            "unit_id": unit.unit_id,
            "name": unit.name,
            "cost": unit.cost,
            "position": unit.position.value,
            "traits": list(unit.traits),
            "trait_ids": list(unit.trait_ids),
            "costs": list(unit.costs or (unit.cost,)),
            "roles": list(unit.roles),
            "portrait_path": unit.portrait_path,
            "description": unit.description,
            "affordable": unit.cost <= self.environment.gold,
        }

    def _unit_view(self, instance: UnitInstance) -> dict[str, object]:
        unit = self.pack.units[instance.unit_id]
        return {
            "unit_id": unit.unit_id,
            "name": unit.name,
            "star": instance.star,
            "cost": unit.cost,
            "position": unit.position.value,
            "traits": list(unit.traits),
            "trait_ids": list(unit.trait_ids),
            "costs": list(unit.costs or (unit.cost,)),
            "roles": list(unit.roles),
            "portrait_path": unit.portrait_path,
            "description": unit.description,
        }

    def _slot_views(
        self,
        slots: list[UnitInstance | None],
    ) -> list[dict[str, object] | None]:
        return [None if unit is None else self._unit_view(unit) for unit in slots]

    def _active_trait_view(self, observation: RunObservation) -> list[dict[str, object]]:
        counts: dict[str, int] = {}
        for unit in observation.active_units:
            definition = self.pack.units[unit.unit_id]
            for trait_id in definition.trait_ids:
                counts[trait_id] = counts.get(trait_id, 0) + 1
        traits = []
        for trait_id, count in counts.items():
            definition = self.pack.traits.get(trait_id)
            if definition is None:
                continue
            active_tier = None
            next_tier = None
            for tier in definition.tiers:
                if tier.level <= count:
                    active_tier = tier
                elif next_tier is None:
                    next_tier = tier
            traits.append(
                {
                    "id": definition.trait_id,
                    "name": definition.name,
                    "group": definition.group,
                    "icon_path": definition.icon_path,
                    "count": count,
                    "active_tier": (None if active_tier is None else active_tier.level),
                    "next_tier": None if next_tier is None else next_tier.level,
                    "active_text": "" if active_tier is None else active_tier.text,
                    "next_text": "" if next_tier is None else next_tier.text,
                    "status": "active" if active_tier is not None else "inactive",
                }
            )
        return sorted(traits, key=lambda trait: (-int(trait["count"]), str(trait["name"])))

    @staticmethod
    def _investment_view(investment: Investment) -> dict[str, object]:
        return {
            "id": investment.investment_id,
            "name": investment.name,
            "rarity": investment.rarity.value,
            "description": investment.description,
        }

    @staticmethod
    def _affix_view(affix: Affix) -> dict[str, object]:
        return {
            "id": affix.affix_id,
            "name": affix.name,
            "description": affix.description,
        }

    def _environment_view(self) -> dict[str, object]:
        environment_id = self.environment.environment_id
        if environment_id is None:
            raise RuntimeError("对局没有初始化投资环境")
        environment = self.pack.environments[environment_id]
        return {
            "id": environment.environment_id,
            "name": environment.name,
            "description": environment.description,
        }

    def _difficulty_view(self) -> dict[str, object]:
        difficulty = self.pack.difficulties.get(self.environment.difficulty_id)
        return {
            "id": self.environment.difficulty_id,
            "name": "自定义" if difficulty is None else difficulty.name,
            "enemy_difficulty": self.environment.enemy_difficulty,
            "affix_count": self.environment.affix_count,
        }

    def _debug_view(self, observation: RunObservation) -> dict[str, object]:
        pool = [
            {
                "unit_id": unit_id,
                "name": self.pack.units[unit_id].name,
                "cost": self.pack.units[unit_id].cost,
                "remaining": remaining,
            }
            for unit_id, remaining in sorted(
                self.environment.market.pool.items(),
                key=lambda item: (self.pack.units[item[0]].cost, item[0]),
            )
        ]
        node = self.environment.current_node if not observation.ended else None
        return {
            "team_power": observation.team_power,
            "node_enemy_rating": (None if node is None else self.environment.enemy_rating(node)),
            "pool": pool,
        }

    @staticmethod
    def _node_result_view(result: NodeResult) -> dict[str, object]:
        return {
            "node_index": result.node_index,
            "node_type": result.node_type.value,
            "won": result.won,
            "battle_progress": result.battle_progress,
            "battle_provider": result.battle_provider,
            "team_power": round(result.team_power, 3),
            "enemy_rating": round(result.enemy_rating, 3),
            "health_delta": result.health_delta,
            "gold_delta": result.gold_delta,
            "streak_after": result.streak_after,
        }


class OperationService:
    """管理多个内存中的运营对局，供 HTTP 适配层调用。"""

    def __init__(
        self,
        pack: RulePack | None = None,
        *,
        asset_root: Path | None = None,
    ) -> None:
        self.pack = load_rule_pack() if pack is None else pack
        self.asset_root = asset_root
        self._sessions: dict[str, OperationSession] = {}
        self._lock = threading.RLock()

    def meta(self) -> dict[str, object]:
        with self._lock:
            template = OperationSession(
                session_id="meta",
                pack=self.pack,
                seed=0,
                battle_progress=FixedBattleProgressProvider(),
                environment=OperationEnvironment(
                    pack=self.pack,
                    rng=random.Random(0),
                    difficulty_id=next(iter(self.pack.difficulties)),
                    environment_id=next(iter(self.pack.environments)),
                    affix_ids=(),
                ),
            )
            return template.meta()

    def create_session(self, payload: object) -> dict[str, object]:
        """验证创建参数并保存新对局。"""

        data = _object(payload)
        difficulty_id = _string(data, "difficulty_id", default=next(iter(self.pack.difficulties)))
        enemy_value = data.get("enemy_difficulty")
        affix_value_count = data.get("affix_count")
        enemy_difficulty = (
            None if enemy_value is None else _required_int(enemy_value, "enemy_difficulty")
        )
        affix_count = (
            None if affix_value_count is None else _required_int(affix_value_count, "affix_count")
        )
        if enemy_difficulty is not None or affix_count is not None:
            defaults = self.pack.difficulties.get(difficulty_id)
            if defaults is None:
                defaults = next(iter(self.pack.difficulties.values()))
            enemy_difficulty = (
                defaults.enemy_difficulty if enemy_difficulty is None else enemy_difficulty
            )
            affix_count = defaults.affix_count if affix_count is None else affix_count
            difficulty_id = "custom"
        environment_value = data.get("environment_id")
        environment_id = (
            None
            if environment_value in (None, "")
            else _required_string(environment_value, "environment_id")
        )
        seed = _integer(data, "seed", default=7)
        progress = _integer(data, "battle_progress", default=100)
        affix_value = data.get("affix_ids")
        affix_ids: tuple[str, ...] | None
        if affix_value is None:
            affix_ids = None
        else:
            if not isinstance(affix_value, list):
                raise SessionInputError("affix_ids 必须是数组")
            affix_ids = tuple(_required_string(item, "affix_ids[]") for item in affix_value)
        try:
            session = OperationSession.create(
                self.pack,
                difficulty_id=difficulty_id,
                environment_id=environment_id,
                seed=seed,
                affix_ids=affix_ids,
                battle_progress=progress,
                enemy_difficulty=enemy_difficulty,
                affix_count=affix_count,
            )
        except ValueError as error:
            raise SessionInputError(str(error)) from error
        with self._lock:
            self._sessions[session.session_id] = session
        return session.state()

    def get_session(self, session_id: str) -> dict[str, object]:
        with self._lock:
            return self._session(session_id).state()

    def apply_action(self, session_id: str, payload: object) -> dict[str, object]:
        data = _object(payload)
        with self._lock:
            session = self._session(session_id)
            try:
                session.apply(data)
            except ValueError as error:
                raise SessionInputError(str(error)) from error
            return session.state()

    def _session(self, session_id: str) -> OperationSession:
        session = self._sessions.get(session_id)
        if session is None:
            raise SessionNotFoundError("对局不存在")
        return session

    def read_asset(self, request_path: str) -> bytes | None:
        """读取数据仓库中的图鉴资源。"""

        if self.asset_root is None or not request_path.startswith("/assets/"):
            return None
        relative = request_path.removeprefix("/")
        path = (self.asset_root / relative).resolve()
        try:
            path.relative_to(self.asset_root.resolve())
        except ValueError:
            return None
        if not path.is_file():
            return None
        return path.read_bytes()


def _object(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise SessionInputError("请求内容必须是对象")
    return value


def _string(
    data: dict[str, object],
    key: str,
    *,
    default: str | None = None,
) -> str:
    value = data.get(key, default)
    return _required_string(value, key)


def _required_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise SessionInputError(f"{label} 必须是非空字符串")
    return value


def _integer(
    data: dict[str, object],
    key: str,
    *,
    default: int | None = None,
) -> int:
    value = data.get(key, default)
    if not isinstance(value, int) or isinstance(value, bool):
        raise SessionInputError(f"{key} 必须是整数")
    return value


def _required_int(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise SessionInputError(f"{label} 必须是整数")
    return value


def _slot_reference(value: object) -> tuple[str, int]:
    data = _object(value)
    return _string(data, "location"), _integer(data, "index")


def _boolean(data: dict[str, object], key: str) -> bool:
    value = data.get(key)
    if not isinstance(value, bool):
        raise SessionInputError(f"{key} 必须是布尔值")
    return value
