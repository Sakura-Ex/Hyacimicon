"""网页会话服务与 HTTP 入口。"""

import json
import threading
import unittest
from urllib.request import Request, urlopen

from research.operations.catalog import load_rule_pack
from research.operations.session import (
    OperationService,
    SessionInputError,
)
from research.operations.web import create_server


class OperationServiceTests(unittest.TestCase):
    """验证网页动作与运营环境使用同一套规则。"""

    def setUp(self):
        self.service = OperationService(load_rule_pack())

    def _create(self, progress: int = 100) -> dict[str, object]:
        return self.service.create_session(
            {
                "difficulty_id": "training",
                "environment_id": "silver_age",
                "seed": 31,
                "battle_progress": progress,
            }
        )

    def test_new_session_contains_manual_operation_state(self):
        session = self._create()

        self.assertEqual(session["phase"], "operations")
        self.assertFalse(session["finished"])
        self.assertEqual(session["battle"]["progress_percent"], 100)
        self.assertEqual(len(session["shop"]), 5)
        self.assertEqual(len(session["node_template"]), 20)
        self.assertTrue(session["events"])

    def test_manual_reroll_and_progress_update(self):
        session = self._create()
        session_id = str(session["session_id"])
        resources = session["resources"]
        self.assertIsInstance(resources, dict)
        gold_before = resources["gold"]

        session = self.service.apply_action(session_id, {"type": "reroll_shop"})
        resources = session["resources"]
        self.assertIsInstance(resources, dict)
        self.assertEqual(resources["gold"], gold_before - 2)

        session = self.service.apply_action(
            session_id,
            {"type": "set_battle_progress", "progress": 65},
        )
        battle = session["battle"]
        self.assertIsInstance(battle, dict)
        self.assertEqual(battle["progress_percent"], 65)

    def test_progress_below_one_hundred_resolves_as_loss(self):
        session = self._create(progress=40)
        session_id = str(session["session_id"])

        session = self.service.apply_action(session_id, {"type": "advance_node"})

        results = session["results"]
        self.assertIsInstance(results, list)
        self.assertEqual(len(results), 1)
        result = results[0]
        self.assertIsInstance(result, dict)
        self.assertFalse(result["won"])
        self.assertEqual(result["battle_progress"], 40)

    def test_investment_phase_uses_same_action_endpoint(self):
        session = self._create()
        session_id = str(session["session_id"])
        for _ in range(2):
            session = self.service.apply_action(session_id, {"type": "advance_node"})

        self.assertEqual(session["phase"], "investment")
        investments = session["investments"]
        self.assertIsInstance(investments, dict)
        offers = investments["offers"]
        self.assertIsInstance(offers, list)
        self.assertTrue(offers)

        session = self.service.apply_action(
            session_id,
            {"type": "choose_investment", "index": 0},
        )
        self.assertEqual(session["phase"], "operations")

    def test_invalid_progress_is_rejected(self):
        session = self._create()
        with self.assertRaises(SessionInputError):
            self.service.apply_action(
                str(session["session_id"]),
                {"type": "set_battle_progress", "progress": 101},
            )

    def test_custom_difficulty_and_manual_deployment_actions(self):
        session = self.service.create_session(
            {
                "difficulty_id": "training",
                "environment_id": "silver_age",
                "seed": 14,
                "enemy_difficulty": 100,
                "affix_count": 3,
                "battle_progress": 100,
            }
        )
        session_id = str(session["session_id"])
        difficulty = session["difficulty"]
        self.assertIsInstance(difficulty, dict)
        self.assertEqual(difficulty["enemy_difficulty"], 100)
        self.assertEqual(difficulty["affix_count"], 3)

        active = session["active_units"]
        self.assertTrue(active)
        first = active[0]
        self.assertIsInstance(first, dict)
        session = self.service.apply_action(
            session_id,
            {
                "type": "set_unit_position",
                "unit_id": first["unit_id"],
                "star": first["star"],
                "position": None,
            },
        )
        self.assertIn(
            first["unit_id"],
            [unit["unit_id"] for unit in session["bench_units"]],
        )
        session = self.service.apply_action(
            session_id,
            {
                "type": "set_unit_position",
                "unit_id": first["unit_id"],
                "star": first["star"],
                "position": "front",
            },
        )
        self.assertIn(
            first["unit_id"],
            [unit["unit_id"] for unit in session["front_units"]],
        )


class OperationHTTPServerTests(unittest.TestCase):
    """验证静态页面和 API 能通过本地 HTTP 使用。"""

    def setUp(self):
        self.server = create_server("127.0.0.1", 0)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        host, port = self.server.server_address
        self.base_url = f"http://{host}:{port}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    def test_index_and_meta_are_served(self):
        with urlopen(f"{self.base_url}/", timeout=2) as response:
            page = response.read().decode("utf-8")
        with urlopen(f"{self.base_url}/api/meta", timeout=2) as response:
            meta = json.load(response)

        self.assertIn("货币战争运营实验台", page)
        self.assertIn("enemy-difficulty", page)
        self.assertIn("affix-count", page)
        self.assertIn("toggle-shop-view", page)
        self.assertIn("bench-panel", page)
        self.assertIn('data-drop-position="front"', page)
        self.assertIn('data-drop-position="back"', page)
        self.assertEqual(meta["game_version"], "4.7.0")
        self.assertTrue(meta["difficulties"])
        self.assertEqual(meta["difficulty_limits"]["enemy_max"], 211)

    def test_create_session_over_http(self):
        request = Request(
            f"{self.base_url}/api/sessions",
            data=json.dumps(
                {
                    "difficulty_id": "training",
                    "environment_id": "silver_age",
                    "seed": 5,
                    "battle_progress": 100,
                }
            ).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlopen(request, timeout=2) as response:
            session = json.load(response)

        self.assertEqual(response.status, 201)
        self.assertEqual(session["phase"], "operations")
        self.assertEqual(session["battle"]["progress_percent"], 100)


if __name__ == "__main__":
    unittest.main()
