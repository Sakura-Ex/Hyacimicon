"""商店、升级和角色合成规则。"""

import random
import unittest

from research.operations.catalog import load_rule_pack
from research.operations.market import Market, MarketActionError, Roster
from research.operations.models import ShopOffer


class MarketTests(unittest.TestCase):
    """验证市场操作和核心状态变化。"""

    def setUp(self):
        self.pack = load_rule_pack()

    def test_three_copies_merge_without_returning_cards(self):
        roster = Roster()

        first = roster.add("herta")
        second = roster.add("herta")
        third = roster.add("herta")

        self.assertEqual(first, ())
        self.assertEqual(second, ())
        self.assertEqual(third, ("herta:2",))
        self.assertEqual(roster.units[("herta", 2)], 1)
        self.assertNotIn(("herta", 1), roster.units)

    def test_purchase_consumes_pool_gold_and_offer(self):
        market = Market(self.pack.rules, self.pack.units, random.Random(1))
        market.offers = (ShopOffer("herta"),)
        initial_pool = market.pool["herta"]

        remaining_gold, merges = market.buy_unit(0, 10)

        self.assertEqual(remaining_gold, 9)
        self.assertEqual(merges, ())
        self.assertEqual(market.pool["herta"], initial_pool - 1)
        self.assertIsNone(market.offers[0].unit_id)
        self.assertEqual(market.roster.units[("herta", 1)], 1)

    def test_sell_returns_consumed_copies_to_pool(self):
        market = Market(self.pack.rules, self.pack.units, random.Random(2))
        initial_pool = market.pool["herta"]
        market.offers = (ShopOffer("herta"),)
        market.buy_unit(0, 10)

        gold = market.sell_unit("herta", 1, 7)

        self.assertEqual(gold, 8)
        self.assertEqual(market.pool["herta"], initial_pool)
        self.assertEqual(market.roster.instances(), ())

    def test_buying_xp_uses_central_threshold_table(self):
        market = Market(self.pack.rules, self.pack.units, random.Random(3))

        gold, levels = market.buy_xp(10)

        self.assertEqual(gold, 9)
        self.assertEqual(levels, (4,))
        self.assertEqual(market.xp, 0)

    def test_manual_refresh_uses_free_then_paid_refresh(self):
        market = Market(self.pack.rules, self.pack.units, random.Random(4))
        market.free_rerolls = 1

        gold, paid = market.refresh_with_resources(10)
        self.assertEqual((gold, paid), (10, False))
        self.assertEqual(market.free_rerolls, 0)

        gold, paid = market.refresh_with_resources(10)
        self.assertEqual((gold, paid), (8, True))

    def test_insufficient_gold_is_reported(self):
        with self.assertRaisesRegex(MarketActionError, "金币不足"):
            market = Market(self.pack.rules, self.pack.units, random.Random(5))
            market.offers = (ShopOffer("herta"),)
            market.buy_unit(0, 0)


if __name__ == "__main__":
    unittest.main()
