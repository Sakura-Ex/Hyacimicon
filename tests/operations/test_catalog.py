"""规则包加载与静态数据约束。"""

import unittest
from collections import Counter

from research.operations.catalog import load_rule_pack
from research.operations.models import NodeType


class RulePackTests(unittest.TestCase):
    """验证规则包版本、基础数量和节点统计。"""

    def test_default_pack_matches_current_schema(self):
        pack = load_rule_pack()

        self.assertEqual(pack.rules.schema_version, 1)
        self.assertEqual(pack.rules.game_version, "4.7.0")
        self.assertEqual(pack.metadata.local_client_build, "20261029-0309-V4.7Live")
        self.assertGreaterEqual(len(pack.units), 20)
        self.assertGreaterEqual(len(pack.affixes), 10)
        self.assertGreaterEqual(len(pack.investments), 10)

    def test_node_template_has_publicly_recorded_counts(self):
        pack = load_rule_pack()
        counts = Counter(pack.rules.node_types)

        self.assertEqual(len(pack.rules.node_types), 20)
        self.assertEqual(counts[NodeType.REWARD], 5)
        self.assertEqual(counts[NodeType.BATTLE], 9)
        self.assertEqual(counts[NodeType.ENCOUNTER], 3)
        self.assertEqual(counts[NodeType.BOSS], 3)

    def test_shop_odds_each_level_sum_to_one(self):
        pack = load_rule_pack()

        for level, odds in enumerate(pack.rules.shop_odds, start=1):
            with self.subTest(level=level):
                self.assertAlmostEqual(sum(odds), 1.0)


if __name__ == "__main__":
    unittest.main()
