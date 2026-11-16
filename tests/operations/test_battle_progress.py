"""战斗进度接口与节点结算。"""

import random
import unittest

from research.operations.battle_progress import (
    BattleContext,
    FixedBattleProgressProvider,
)
from research.operations.catalog import load_rule_pack
from research.operations.environment import OperationEnvironment
from research.operations.models import NodeType
from research.operations.policies import run_greedy


class BattleProgressTests(unittest.TestCase):
    """验证固定进度、输入约束和节点胜负。"""

    def setUp(self):
        self.pack = load_rule_pack()

    def _environment(self, provider: FixedBattleProgressProvider) -> OperationEnvironment:
        return OperationEnvironment(
            pack=self.pack,
            rng=random.Random(21),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
            battle_progress=provider,
        )

    def test_default_progress_wins_the_first_battle(self):
        env = self._environment(FixedBattleProgressProvider())

        result = env.advance_node()

        self.assertTrue(result.won)
        self.assertEqual(result.battle_progress, 100)
        self.assertEqual(result.battle_provider, "fixed")
        self.assertEqual(result.health_delta, 0)
        self.assertEqual(env.streak, 1)

    def test_progress_below_one_hundred_fails_the_node(self):
        provider = FixedBattleProgressProvider(99)
        env = self._environment(provider)

        result = env.advance_node()

        self.assertFalse(result.won)
        self.assertEqual(result.battle_progress, 99)
        self.assertLess(result.health_delta, 0)
        self.assertEqual(env.streak, 0)

    def test_provider_receives_current_battle_context(self):
        captured: list[BattleContext] = []

        class CapturingProvider:
            @property
            def provider_id(self) -> str:
                return "capturing"

            def resolve(self, context: BattleContext):
                captured.append(context)
                from research.operations.battle_progress import BattleProgressResult

                return BattleProgressResult(100, self.provider_id)

        env = self._environment(CapturingProvider())
        env.advance_node()

        self.assertEqual(len(captured), 1)
        context = captured[0]
        self.assertEqual(context.node_type, NodeType.REWARD)
        self.assertEqual(context.level, 3)
        self.assertEqual(context.difficulty_id, "training")
        self.assertTrue(context.active_units)

    def test_progress_rejects_invalid_values(self):
        for value in (-1, 101, True, 99.5, "100"):
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, "0 到 100"):
                    FixedBattleProgressProvider(value)

    def test_automated_strategy_accepts_progress_injection(self):
        environment = run_greedy(
            self.pack,
            difficulty_id="training",
            environment_id="silver_age",
            seed=8,
            battle_progress=0,
        )

        self.assertTrue(environment.results)
        self.assertTrue(all(result.battle_progress == 0 for result in environment.results))


if __name__ == "__main__":
    unittest.main()
