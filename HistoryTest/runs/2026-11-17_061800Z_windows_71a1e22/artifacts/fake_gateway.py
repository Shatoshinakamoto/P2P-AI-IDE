"""fake_gateway.py -- a tiny stand-in gateway on 127.0.0.1 for the IDE's Windows check (phase I2a). Dev test only.

Serves GET /v1/models and POST /v1/chat/completions (stream) like a P2P-AI gateway, and requires
Authorization: Bearer <the key given on the command line>. Plain http on loopback only, which the IDE accepts as
INSECURE - DEV ONLY. Logs one line per request with the path and whether the key matched (never the key itself).
Usage: python fake_gateway.py PORT KEY
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT, KEY = int(sys.argv[1]), sys.argv[2]


class H(BaseHTTPRequestHandler):
    def _authed(self):
        ok = self.headers.get("Authorization") == f"Bearer {KEY}"
        print(time.strftime("%H:%M:%S"), self.command, self.path, "key-ok" if ok else "key-bad", flush=True)
        if not ok:
            self.send_response(401)
            self.end_headers()
        return ok

    def do_GET(self):
        if not self._authed():
            return
        if self.path != "/v1/models":
            self.send_response(404)
            self.end_headers()
            return
        body = json.dumps({"object": "list", "data": [{"id": "fake-small"}, {"id": "fake-large"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        req = json.loads(self.rfile.read(n) or b"{}")
        if not self._authed():
            return
        last = req.get("messages", [{}])[-1].get("content", "")
        print("  model", req.get("model"), "messages", len(req.get("messages", [])), "stream", req.get("stream"), flush=True)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        words = f"You said {len(last)} characters. Here is code:\n```python\nprint('hi')\n```\n<b>not bold</b> done.".split(" ")
        for w in words:
            chunk = {"object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {"content": w + " "}, "finish_reason": None}]}
            self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
            self.wfile.flush()
            time.sleep(0.08)
        self.wfile.write(b'data: {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n')

    def log_message(self, *a):
        pass


ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
