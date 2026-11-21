"""Fake Ollama for the I7 end-to-end test (test-only, loopback only).

Answers /api/tags, /api/ps, /api/generate and /api/chat. A chat with tools and a user turn last asks for
read_file; a chat whose last turn is a tool result answers with text naming it; a prompt containing "slow"
waits 7 s so TandemForge's SSE keep-alives are exercised. Every request body is appended to a JSON-lines log.
"""
import json, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = sys.argv[2]
MODELS = [{"name": "tiny-chat:1b", "model": "tiny-chat:1b", "size": 800_000_000, "details": {"families": ["llama"]}},
          {"name": "nomic-embed-text:latest", "model": "nomic-embed-text:latest", "size": 270_000_000, "details": {"families": ["nomic-bert"]}}]

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, obj):
        b = json.dumps(obj).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        if self.path == "/api/tags": return self.send({"models": MODELS})
        if self.path == "/api/ps": return self.send({"models": []})
        self.send_response(404); self.end_headers()
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"{}")
        with open(LOG, "a") as fh: fh.write(json.dumps({"path": self.path, "body": body}) + "\n")
        if self.path == "/api/generate": return self.send({"done": True})
        if self.path != "/api/chat": self.send_response(404); self.end_headers(); return
        msgs = body.get("messages") or []
        last = msgs[-1] if msgs else {}
        if "slow" in (last.get("content") or ""): time.sleep(7)
        if body.get("tools") and last.get("role") == "user":
            msg = {"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "read_file", "arguments": {"path": "README.md"}}}]}
        elif last.get("role") == "tool":
            msg = {"role": "assistant", "content": "The file says: " + (last.get("content") or "")[:40]}
        else:
            msg = {"role": "assistant", "content": "Hello from tiny-chat <b>on this computer</b>."}
        self.send({"model": body.get("model"), "message": msg, "done": True, "eval_count": 5, "eval_duration": 1_000_000})

ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
