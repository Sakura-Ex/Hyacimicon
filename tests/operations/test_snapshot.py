"""私有数据仓库图鉴快照接入。"""

import unittest
from pathlib import Path

from research.operations.models import NodeType
from research.operations.session import OperationService
from research.operations.snapshot import (
    load_snapshot_rule_pack,
    locate_data_repository,
)

DATA_REPOSITORY = Path("E:/hsr-ai/hsr-currency-wars-data")


@unittest.skipUnless(
    (DATA_REPOSITORY / "data" / "encyclopedia" / "manifest.json").is_file(),
    "缺少本地 hsr-currency-wars-data 数据仓库",
)
class SnapshotRulePackTests(unittest.TestCase):
    """验证最新角色、羁绊与节点数据能转换为运行时规则。"""

    @classmethod
    def setUpClass(cls):
        cls.pack = load_snapshot_rule_pack(DATA_REPOSITORY)

    def test_snapshot_metadata_and_counts(self):
        self.assertEqual(self.pack.metadata.snapshot_id, "encyclopedia-4.6.54-85a54e5115f8")
        self.assertEqual(len(self.pack.units), 61)
        self.assertEqual(len(self.pack.traits), 38)
        self.assertEqual(self.pack.rules.game_version, "4.6.54")

    def test_latest_character_fields_are_preserved(self):
        aha = self.pack.units["characters-001"]

        self.assertEqual(aha.name, "星神★阿哈")
        self.assertEqual(aha.costs, (4, 5, 7))
        self.assertEqual(aha.cost, 4)
        self.assertEqual(aha.roles, ("辅助",))
        self.assertEqual(aha.traits, ("欢愉", "幻乐之主"))
        self.assertEqual(aha.portrait_path, "/assets/encyclopedia/characters/1511.webp")

    def test_trait_tiers_and_icons_are_loaded(self):
        trait = self.pack.traits["traits-004"]

        self.assertEqual(trait.name, "奇兽花园")
        self.assertEqual(trait.group, "阵营")
        self.assertEqual([tier.level for tier in trait.tiers], [3, 5, 7, 10])
        self.assertEqual(
            trait.icon_path,
            "/assets/encyclopedia/traits/100202.webp",
        )

    def test_current_node_order_and_types_are_loaded(self):
        self.assertEqual(
            self.pack.rules.node_ids,
            (
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
        )
        self.assertEqual(
            self.pack.rules.node_types[0:3],
            (
                NodeType.REWARD,
                NodeType.REWARD,
                NodeType.BATTLE,
            ),
        )

    def test_repository_is_discovered_from_workspace(self):
        self.assertEqual(
            locate_data_repository(Path("E:/hsr-ai")),
            DATA_REPOSITORY,
        )

    def test_web_session_uses_snapshot_units_and_assets(self):
        service = OperationService(self.pack, asset_root=DATA_REPOSITORY)
        session = service.create_session(
            {
                "difficulty_id": "training",
                "environment_id": "silver_age",
                "seed": 13,
                "battle_progress": 100,
            }
        )

        self.assertEqual(session["snapshot_id"], self.pack.metadata.snapshot_id)
        self.assertEqual(len(session["node_template"]), 20)
        self.assertTrue(session["shop"])
        self.assertTrue(any(offer.get("portrait_path") for offer in session["shop"]))
        self.assertIsNotNone(service.read_asset("/assets/encyclopedia/characters/1511.webp"))
