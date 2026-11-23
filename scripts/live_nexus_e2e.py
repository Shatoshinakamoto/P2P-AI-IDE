#!/usr/bin/env python3
"""live_nexus_e2e.py -- real NEXUS Proxy + NEXUS Runtime end-to-end check against P2P-AI-IDE core.

Builds/runs `nexus-proxy` and `nexus-runtime` from a local `nexus-engine` checkout on loopback `127.0.0.1`,
plus a deterministic test-only OpenAI SSE upstream on `127.0.0.1` for tool-calling forwarding, and runs
`cargo test --test nexus_engine -- --ignored --nocapture` in `core/`.

Security controls:
  - Fixed argument lists (`shell=False`, CWE-78).
  - Strictly binds all servers to `127.0.0.1` (NIST SC-7, CWE-918).
  - Clean 0700 temp folder removed on exit unless `--keep` is passed; test secrets never leave local disk.
"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
CORE = os.path.join(ROOT, "core")


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_health(url, expected_service, timeout=20.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"{url}/health", timeout=1.5) as resp:
                if 200 <= resp.status < 300:
                    doc = json.loads(resp.read().decode("utf-8"))
                    if doc.get("status") == "ok" and doc.get("service") == expected_service:
                        return
        except (OSError, urllib.error.URLError, ValueError):
            time.sleep(0.2)
    raise RuntimeError(f"timed out waiting for {expected_service} at {url}/health")


class ScriptedOpenAIHandler(BaseHTTPRequestHandler):
    saw_secret = False

    def log_message(self, format, *args):
        return

    def do_GET(self):
        if self.path == "/v1/models":
            body = json.dumps({"object": "list", "data": [{"id": "nexus-e2e-coder"}]}).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        if self.path != "/v1/chat/completions":
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length).decode("utf-8")
        if "never_send_me" in raw:
            ScriptedOpenAIHandler.saw_secret = True
        req = json.loads(raw)
        model = req.get("model", "nexus-e2e-coder")
        msgs = req.get("messages", [])
        has_tools = bool(req.get("tools"))
        tool_msgs = [m for m in msgs if m.get("role") == "tool"]

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

        def emit(chunk_obj):
            line = f"data: {json.dumps(chunk_obj)}\n\n".encode("utf-8")
            self.wfile.write(line)
            self.wfile.flush()

        if not has_tools:
            emit({"id": "c1", "model": model, "choices": [{"index": 0, "delta": {"content": "Packed via live NEXUS proxy."}}]})
            emit({"id": "c1", "model": model, "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]})
        elif len(tool_msgs) == 0:
            emit({
                "id": "c2",
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {"tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"app.py\"}"}}]},
                    "finish_reason": None,
                }],
            })
            emit({"id": "c2", "model": model, "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]})
        elif len(tool_msgs) == 1:
            emit({
                "id": "c3",
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {"tool_calls": [{"index": 0, "id": "call_2", "type": "function", "function": {"name": "edit_file", "arguments": "{\"path\":\"app.py\",\"old_text\":\"return \\\"old\\\"\",\"new_text\":\"return \\\"fixed\\\"\"}"}}]},
                    "finish_reason": None,
                }],
            })
            emit({"id": "c3", "model": model, "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]})
        else:
            emit({"id": "c4", "model": model, "choices": [{"index": 0, "delta": {"content": "Proposed greeting update in app.py."}}]})
            emit({"id": "c4", "model": model, "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]})

        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()


def main():
    p = argparse.ArgumentParser(description="Run live E2E test against real nexus-proxy & nexus-runtime")
    p.add_argument("--nexus-engine", required=True, help="path to nexus-engine checkout")
    p.add_argument("--keep", action="store_true", help="keep temp work folder on exit")
    args = p.parse_args()

    engine_dir = os.path.abspath(args.nexus_engine)
    if not os.path.isfile(os.path.join(engine_dir, "Cargo.toml")):
        print(f"error: not a nexus-engine checkout: {engine_dir}", file=sys.stderr)
        return 2

    print("Building nexus-proxy and nexus-runtime binaries...")
    build = subprocess.run(
        ["cargo", "build", "--bin", "nexus-proxy", "--bin", "nexus-runtime"],
        cwd=engine_dir,
    )
    if build.returncode != 0:
        return build.returncode

    proxy_bin = os.path.join(engine_dir, "target", "debug", "nexus-proxy")
    runtime_bin = os.path.join(engine_dir, "target", "debug", "nexus-runtime")
    if os.name == "nt":
        proxy_bin += ".exe"
        runtime_bin += ".exe"

    work = tempfile.mkdtemp(prefix="p2pide-live-nexus-")
    os.chmod(work, 0o700)
    procs = []
    httpd = None
    try:
        models_dir = os.path.join(work, "models")
        os.makedirs(models_dir, exist_ok=True)
        rt_port = free_port()
        rt_url = f"http://127.0.0.1:{rt_port}"
        rt_log = open(os.path.join(work, "nexus-runtime.log"), "wb")
        procs.append(
            subprocess.Popen(
                [runtime_bin, "serve", "--host", "127.0.0.1", "--port", str(rt_port), "--models-dir", models_dir],
                cwd=work,
                stdout=rt_log,
                stderr=subprocess.STDOUT,
            )
        )
        wait_health(rt_url, "nexus-runtime")
        print(f"nexus-runtime ready at {rt_url}")

        # 1. Mock-ollama proxy run
        mock_port = free_port()
        mock_url = f"http://127.0.0.1:{mock_port}"
        mock_log = open(os.path.join(work, "nexus-proxy-mock.log"), "wb")
        procs.append(
            subprocess.Popen(
                [
                    proxy_bin,
                    "--host", "127.0.0.1",
                    "--port", str(mock_port),
                    "--db-path", os.path.join(work, "mock.db"),
                    "--mock-ollama",
                ],
                cwd=work,
                stdout=mock_log,
                stderr=subprocess.STDOUT,
            )
        )
        wait_health(mock_url, "nexus-proxy")
        print(f"nexus-proxy (mock mode) ready at {mock_url}")

        env1 = os.environ.copy()
        env1["NEXUS_PROXY_URL"] = mock_url
        env1["NEXUS_RUNTIME_URL"] = rt_url
        env1["NEXUS_AGENT_TOOLS"] = "0"
        r1 = subprocess.run(
            ["cargo", "test", "--locked", "--test", "nexus_engine", "--", "--ignored", "--nocapture"],
            cwd=CORE,
            env=env1,
        )
        if r1.returncode != 0:
            return r1.returncode

        # 2. Live forwarding + agent tool calls proxy run
        up_port = free_port()
        httpd = ThreadingHTTPServer(("127.0.0.1", up_port), ScriptedOpenAIHandler)
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        live_port = free_port()
        live_url = f"http://127.0.0.1:{live_port}"
        live_log = open(os.path.join(work, "nexus-proxy-live.log"), "wb")
        procs.append(
            subprocess.Popen(
                [
                    proxy_bin,
                    "--host", "127.0.0.1",
                    "--port", str(live_port),
                    "--db-path", os.path.join(work, "live.db"),
                    "--ollama-url", f"http://127.0.0.1:{up_port}",
                ],
                cwd=work,
                stdout=live_log,
                stderr=subprocess.STDOUT,
            )
        )
        wait_health(live_url, "nexus-proxy")
        print(f"nexus-proxy (live upstream tool forwarding) ready at {live_url}")

        env2 = os.environ.copy()
        env2["NEXUS_PROXY_URL"] = live_url
        env2["NEXUS_RUNTIME_URL"] = rt_url
        env2["NEXUS_AGENT_TOOLS"] = "1"
        r2 = subprocess.run(
            ["cargo", "test", "--locked", "--test", "nexus_engine", "--", "--ignored", "--nocapture"],
            cwd=CORE,
            env=env2,
        )
        if ScriptedOpenAIHandler.saw_secret:
            print("FAIL secret leaked into upstream traffic", file=sys.stderr)
            return 1
        return r2.returncode
    finally:
        if httpd is not None:
            httpd.shutdown()
            httpd.server_close()
        for proc in procs:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
        if not args.keep:
            shutil.rmtree(work, ignore_errors=True)
        else:
            print(f"kept work dir: {work}")


if __name__ == "__main__":
    sys.exit(main())
