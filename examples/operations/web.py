"""启动本地运营模拟操作与检查网页。"""

import argparse
import webbrowser

from research.operations.web import create_server


def main() -> None:
    parser = argparse.ArgumentParser(description="启动货币战争运营模拟网页")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument(
        "--no-open",
        action="store_true",
        help="启动后不自动打开浏览器",
    )
    args = parser.parse_args()

    server = create_server(args.host, args.port)
    host, port = server.server_address
    url = f"http://{host}:{port}/"
    print(f"运营模拟网页已启动：{url}")
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
