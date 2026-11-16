"""整局运营环境、投资策略和开局词缀。"""

import random
import unittest

from research.operations.catalog import load_rule_pack
from research.operations.environment import OperationActionError, OperationEnvironment
from research.operations.models import InvestmentRarity, Position, ShopOffer
from research.operations.policies import run_greedy


class EnvironmentTests(unittest.TestCase):
    """验证可复现性、可见信息、投资和难度词缀。"""

    def setUp(self):
        self.pack = load_rule_pack()

    def test_explicit_bad_start_reduces_initial_health(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(1),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=("bad_start",),
        )

        self.assertEqual(env.max_health, 100)
        self.assertEqual(env.health, 80)

    def test_observation_hides_card_pool(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(2),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        observation = env.observe()

        self.assertFalse(hasattr(observation, "pool"))
        self.assertGreater(len(observation.shop_offers), 0)

    def test_first_prize_forces_a_prismatic_option(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(3),
            difficulty_id="training",
            environment_id="first_prize",
            affix_ids=(),
        )
        while env.node_index < 2:
            env.advance_node()

        self.assertTrue(env.investments.current_offers)
        self.assertTrue(
            any(
                investment.rarity is InvestmentRarity.PRISMATIC
                for investment in env.investments.current_offers
            )
        )

    def test_investment_is_required_before_market_action(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(4),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        while env.node_index < 2:
            env.advance_node()

        with self.assertRaisesRegex(OperationActionError, "投资策略"):
            env.reroll_shop()
        with self.assertRaisesRegex(OperationActionError, "投资策略"):
            env.buy_xp()
        first_unit = env.market.roster.instances()[0]
        with self.assertRaisesRegex(OperationActionError, "投资策略"):
            env.sell_unit(first_unit.unit_id, first_unit.star)

    def test_same_seed_and_actions_reproduce_run(self):
        first = run_greedy(
            self.pack,
            difficulty_id="standard",
            environment_id="silver_age",
            seed=42,
        )
        second = run_greedy(
            self.pack,
            difficulty_id="standard",
            environment_id="silver_age",
            seed=42,
        )

        self.assertEqual(first.won, second.won)
        self.assertEqual(first.health, second.health)
        self.assertEqual(first.gold, second.gold)
        self.assertEqual(first.results, second.results)

    def test_supported_difficulties_produce_complete_runs(self):
        for difficulty_id in self.pack.difficulties:
            with self.subTest(difficulty_id=difficulty_id):
                env = run_greedy(
                    self.pack,
                    difficulty_id=difficulty_id,
                    environment_id="silver_age",
                    seed=9,
                )
                self.assertTrue(env.ended)
                self.assertLessEqual(len(env.results), 20)

    def test_difficulty_changes_enemy_rating(self):
        training = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(5),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        apex = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(5),
            difficulty_id="apex",
            environment_id="silver_age",
            affix_ids=(),
        )

        self.assertGreater(apex.enemy_rating(), training.enemy_rating())

    def test_random_affix_count_comes_from_difficulty(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(12),
            difficulty_id="standard",
            environment_id="silver_age",
        )

        self.assertEqual(len(env.affixes), self.pack.difficulties["standard"].affix_count)

    def test_investment_immediate_effects_update_environment(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(6),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        env.investments.current_offers = (self.pack.investments["streamline"],)
        gold_before = env.gold

        env.choose_investment(0)

        self.assertEqual(env.gold, gold_before + 10)
        self.assertEqual(env.interest_cap, 9)

    def test_all_packaged_environments_produce_complete_runs(self):
        for environment_id in self.pack.environments:
            with self.subTest(environment_id=environment_id):
                env = run_greedy(
                    self.pack,
                    difficulty_id="training",
                    environment_id=environment_id,
                    seed=11,
                )
                self.assertTrue(env.ended)

    def test_starting_units_use_only_one_to_three_cost_characters(self):
        for seed in range(20):
            with self.subTest(seed=seed):
                env = OperationEnvironment(
                    pack=self.pack,
                    rng=random.Random(seed),
                    difficulty_id="training",
                    environment_id="silver_age",
                    affix_ids=(),
                )
                for unit in env.market.roster.instances():
                    self.assertLessEqual(self.pack.units[unit.unit_id].cost, 3)

    def test_custom_difficulty_uses_enemy_level_and_affix_count(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(5),
            difficulty_id="custom",
            environment_id="silver_age",
            enemy_difficulty=100,
            affix_count=5,
        )

        self.assertEqual(env.enemy_difficulty, 100)
        self.assertEqual(env.affix_count, 5)
        self.assertEqual(len(env.affixes), 5)

    def test_front_row_is_limited_to_four_characters(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(8),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        env.market.level = 5
        for unit in env.active_units():
            env.set_unit_position(unit.unit_id, unit.star, None)
        candidates = [
            definition
            for definition in self.pack.units.values()
            if (definition.unit_id, 1) not in env.market.roster.units
        ][:5]
        for definition in candidates:
            env.market.roster.add(definition.unit_id)
        for definition in candidates[:4]:
            env.set_unit_position(definition.unit_id, 1, Position.FRONT)

        self.assertEqual(len(env.front_units()), 4)
        with self.assertRaisesRegex(OperationActionError, "前台已满"):
            env.set_unit_position(candidates[4].unit_id, 1, Position.FRONT)

    def test_purchased_unit_can_be_deployed_and_bench_is_limited(self):
        env = OperationEnvironment(
            pack=self.pack,
            rng=random.Random(10),
            difficulty_id="training",
            environment_id="silver_age",
            affix_ids=(),
        )
        purchased = next(
            definition
            for definition in self.pack.units.values()
            if (definition.unit_id, 1) not in env.market.roster.units
        )
        env.market.offers = (ShopOffer(purchased.unit_id),)
        env.buy_shop_offer(0)
        self.assertIn(purchased.unit_id, [unit.unit_id for unit in env.bench_units()])

        first_active = env.active_units()[0]
        env.set_unit_position(first_active.unit_id, first_active.star, None)
        env.set_unit_position(purchased.unit_id, 1, Position.FRONT)
        self.assertIn(purchased.unit_id, [unit.unit_id for unit in env.front_units()])

        for unit in env.active_units():
            env.set_unit_position(unit.unit_id, unit.star, None)
        for definition in self.pack.units.values():
            if env.bench_count >= self.pack.rules.bench_capacity:
                break
            env.market.roster.add(definition.unit_id)
        blocked = next(
            definition
            for definition in self.pack.units.values()
            if (definition.unit_id, 1) not in env.market.roster.units
            and definition.cost <= env.gold
        )
        env.market.offers = (ShopOffer(blocked.unit_id),)
        with self.assertRaisesRegex(OperationActionError, "备战席已满"):
            env.buy_shop_offer(0)


if __name__ == "__main__":
    unittest.main()
