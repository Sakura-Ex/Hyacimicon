"""运行单局或批量运营模拟。"""

import argparse
import json

from research.operations.catalog import load_rule_pack
from research.operations.policies import run_batch, run_greedy


def main() -> None:
    parser = argparse.ArgumentParser(description="货币战争运营模拟")
    parser.add_argument("--difficulty", default="standard")
    parser.add_argument("--environment", default=None)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument(
        "--affix",
        action="append",
        default=None,
        help="指定词缀 ID，可重复；省略时按难度随机抽取",
    )
    args = parser.parse_args()

    pack = load_rule_pack()
    if args.runs == 1:
        environment = run_greedy(
            pack,
            difficulty_id=args.difficulty,
            environment_id=args.environment,
            seed=args.seed,
            affix_ids=None if args.affix is None else tuple(args.affix),
        )
        payload = {
            "won": environment.won,
            "health": environment.health,
            "gold": environment.gold,
            "level": environment.market.level,
            "node_index": environment.node_index,
            "affixes": [affix.affix_id for affix in environment.affixes],
            "investments": [
                investment.investment_id for investment in environment.investments.selected
            ],
            "results": [
                {
                    "node_index": result.node_index,
                    "node_type": result.node_type,
                    "won": result.won,
                    "win_probability": round(result.win_probability, 4),
                    "team_power": round(result.team_power, 3),
                    "enemy_rating": round(result.enemy_rating, 3),
                    "health_delta": result.health_delta,
                    "gold_delta": result.gold_delta,
                }
                for result in environment.results
            ],
            "validation": pack.metadata.validation_status,
            "assumptions": pack.metadata.assumptions,
        }
    else:
        summary = run_batch(
            pack,
            difficulty_id=args.difficulty,
            environment_id=args.environment,
            runs=args.runs,
            seed=args.seed,
        )
        payload = {
            "runs": summary.runs,
            "wins": summary.wins,
            "win_rate": summary.win_rate,
            "average_nodes": summary.average_nodes,
            "average_health": summary.average_health,
            "average_gold": summary.average_gold,
            "average_level": summary.average_level,
            "validation": pack.metadata.validation_status,
            "assumptions": pack.metadata.assumptions,
        }
    print(json.dumps(payload, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
