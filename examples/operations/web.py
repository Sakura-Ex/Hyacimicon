"""启动本地运营模拟操作与检查网页。"""

import argparse
import webbrowser
from pathlib import Path

from research.operations.session import OperationService
from research.operations.snapshot import (
    load_snapshot_rule_pack,
    locate_data_repository,
)
from research.operations.web import create_server


def main() -> None:
    parser = argparse.ArgumentParser(description="启动货币战争运营模拟网页")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument(
        "--data-root",
        type=Path,
        default=None,
        help="hsr-currency-wars-data 仓库目录；省略时自动查找",
    )
    parser.add_argument(
        "--no-open",
        action="store_true",
        help="启动后不自动打开浏览器",
    )
    args = parser.parse_args()

    data_root = locate_data_repository() if args.data_root is None else args.data_root.resolve()
    pack = load_snapshot_rule_pack(data_root)
    service = OperationService(pack, asset_root=data_root)
    server = create_server(args.host, args.port, service=service)
    host, port = server.server_address
    url = f"http://{host}:{port}/"
    print(f"运营模拟网页已启动：{url}")
    print(f"数据快照：{pack.metadata.snapshot_id}，角色 {len(pack.units)}，羁绊 {len(pack.traits)}")
    if not args.no_open:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
