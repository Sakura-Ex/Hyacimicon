"""本地运营模拟网页服务。"""

import json
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

from .session import (
    OperationService,
    SessionInputError,
    SessionNotFoundError,
)

WEB_ROOT = Path(__file__).with_name("web")
STATIC_FILES = {
    "/": ("index.html", "text/html; charset=utf-8"),
    "/static/app.js": ("app.js", "text/javascript; charset=utf-8"),
    "/static/styles.css": ("styles.css", "text/css; charset=utf-8"),
}


class OperationHTTPServer(ThreadingHTTPServer):
    """保存共享服务，并限制请求线程为后台线程。"""

    daemon_threads = True

    def __init__(
        self,
        server_address: tuple[str, int],
        service: OperationService | None = None,
    ) -> None:
        self.service = OperationService() if service is None else service
        super().__init__(server_address, OperationRequestHandler)


class OperationRequestHandler(BaseHTTPRequestHandler):
    """提供静态网页和 JSON API。"""

    server: OperationHTTPServer

    def do_GET(self) -> None:
        path = urlsplit(self.path).path
        if path in STATIC_FILES:
            filename, content_type = STATIC_FILES[path]
            self._send_bytes(HTTPStatus.OK, (WEB_ROOT / filename).read_bytes(), content_type)
            return
        if path == "/api/meta":
            self._send_json(HTTPStatus.OK, self.server.service.meta())
            return
        if path.startswith("/api/sessions/"):
            session_id = path.removeprefix("/api/sessions/")
            if "/" not in session_id and session_id:
                self._run_api(lambda: self.server.service.get_session(session_id))
                return
        self._send_json(HTTPStatus.NOT_FOUND, {"error": "路径不存在"})

    def do_POST(self) -> None:
        path = urlsplit(self.path).path
        if path == "/api/sessions":
            payload = self._read_json()
            if payload is None:
                return
            self._run_api(
                lambda: self.server.service.create_session(payload),
                success_status=HTTPStatus.CREATED,
            )
            return
        suffix = "/actions"
        if path.startswith("/api/sessions/") and path.endswith(suffix):
            session_id = path.removeprefix("/api/sessions/").removesuffix(suffix)
            if session_id and "/" not in session_id:
                payload = self._read_json()
                if payload is None:
                    return
                self._run_api(lambda: self.server.service.apply_action(session_id, payload))
                return
        self._send_json(HTTPStatus.NOT_FOUND, {"error": "路径不存在"})

    def log_message(self, format: str, *args: object) -> None:
        del format, args

    def _run_api(
        self,
        operation: object,
        *,
        success_status: HTTPStatus = HTTPStatus.OK,
    ) -> None:
        if not callable(operation):
            raise RuntimeError("API 操作必须可调用")
        try:
            payload = operation()
        except SessionNotFoundError as error:
            self._send_json(HTTPStatus.NOT_FOUND, {"error": str(error)})
        except SessionInputError as error:
            self._send_json(HTTPStatus.BAD_REQUEST, {"error": str(error)})
        except OSError as error:
            self._send_json(
                HTTPStatus.INTERNAL_SERVER_ERROR,
                {"error": f"服务器读取失败：{error}"},
            )
        else:
            self._send_json(success_status, payload)

    def _read_json(self) -> object | None:
        content_length = self.headers.get("Content-Length")
        if content_length is None:
            self._send_json(HTTPStatus.LENGTH_REQUIRED, {"error": "缺少请求长度"})
            return None
        try:
            length = int(content_length)
        except ValueError:
            self._send_json(HTTPStatus.BAD_REQUEST, {"error": "请求长度非法"})
            return None
        if length < 0 or length > 1_000_000:
            self._send_json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"error": "请求过大"})
            return None
        try:
            return json.loads(self.rfile.read(length))
        except (json.JSONDecodeError, UnicodeDecodeError):
            self._send_json(HTTPStatus.BAD_REQUEST, {"error": "请求不是合法 JSON"})
            return None

    def _send_json(self, status: HTTPStatus, payload: object) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self._send_bytes(status, body, "application/json; charset=utf-8")

    def _send_bytes(
        self,
        status: HTTPStatus,
        body: bytes,
        content_type: str,
    ) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def create_server(
    host: str = "127.0.0.1",
    port: int = 8765,
    *,
    service: OperationService | None = None,
) -> OperationHTTPServer:
    """创建可嵌入测试或由命令行启动的网页服务。"""

    return OperationHTTPServer((host, port), service)
