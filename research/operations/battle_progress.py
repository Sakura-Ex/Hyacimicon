"""战斗与运营之间的进度接口。"""

from dataclasses import dataclass
from typing import Protocol

from .models import NodeType, UnitInstance


@dataclass(frozen=True)
class BattleContext:
    """交给战斗进度提供器的当前节点与阵容信息。"""

    node_index: int
    node_type: NodeType
    plane: int
    health: int
    gold: int
    level: int
    streak: int
    team_power: float
    enemy_rating: float
    active_units: tuple[UnitInstance, ...]
    difficulty_id: str
    affix_ids: tuple[str, ...]
    investment_ids: tuple[str, ...]


@dataclass(frozen=True)
class BattleProgressResult:
    """一次节点战斗的结果。百分之百表示胜利。"""

    progress_percent: int
    provider_id: str
    detail: str = ""


class BattleProgressProvider(Protocol):
    """可替换的战斗结果来源。"""

    @property
    def provider_id(self) -> str:
        """返回写入回放的提供器标识。"""

    def resolve(self, context: BattleContext) -> BattleProgressResult:
        """根据节点和阵容返回战斗进度。"""


class FixedBattleProgressProvider:
    """返回固定进度的提供器，默认让所有战斗直接胜利。"""

    def __init__(self, progress_percent: int = 100) -> None:
        self._progress_percent = 100
        self.set_progress(progress_percent)

    @property
    def provider_id(self) -> str:
        return "fixed"

    @property
    def progress_percent(self) -> int:
        return self._progress_percent

    def set_progress(self, progress_percent: int) -> None:
        """修改后续战斗节点使用的固定进度。"""

        if (
            not isinstance(progress_percent, int)
            or isinstance(progress_percent, bool)
            or not 0 <= progress_percent <= 100
        ):
            raise ValueError("战斗进度必须是 0 到 100 的整数")
        self._progress_percent = progress_percent

    def resolve(self, context: BattleContext) -> BattleProgressResult:
        del context
        return BattleProgressResult(
            progress_percent=self._progress_percent,
            provider_id=self.provider_id,
        )
