#!/usr/bin/env python3
"""Live check of the IDE's phase I4 (missions) against a real P2P-AI gateway, node and sandbox.

Purpose: the core and UI tests use fakes. This script runs the real parts of a P2P-AI checkout on 127.0.0.1 and
then the IDE core's `live_missions` example against them, the way the Missions view does:

    IDE core (cargo example live_missions)  ->  Go gateway (projects, workspace sync, agent runs, Autopilot, EXEC)
                                                   <- Go node (the project copy, the workspace tool host)
                                                        -> sandbox runner (Docker: no network, read-only, non-root)
                                                        -> scripted test-only model (loopback)

The scripted model plans one task, then reads src/calc.py and writes the fix; the gateway's Autopilot runs the
unittest check in the sandbox. Checks are printed by the example (see core/examples/live_missions.rs); this
script adds one: the project's .env value is in no file the gateway stored.

Usage: python3 scripts/live_gateway_e2e.py --p2p-ai PATH [--bin-dir DIR] [--image NAME] [--keep]
Inputs: a P2P-AI checkout (its Go gateway and node, deploy/mock_ollama.py and scripts/shell_session_stack_e2e.py
helpers); with --bin-dir, prebuilt `gateway` and `p2pnode` binaries; the sandbox image (built beforehand with
`docker build -f deploy/Dockerfile.sandbox -t p2p-ai/sandbox:dev .` in the P2P-AI checkout).
Outputs: PASS/FAIL lines and `SUMMARY: n passed, n failed`; exit 0 (all passed), 1 (a check failed), 2 (setup).
Trust boundaries: every listener is on 127.0.0.1 with an ephemeral port; the runner container has no network.

Security notes:
  - SECURITY: admin, enroll and signing secrets and the API key are generated per run with a CSPRNG (secrets),
    live in a 0700 mkdtemp folder, are passed to children through owner-only files or a scratch environment,
    and are never printed (CWE-532, CWE-798, CWE-214, CWE-526).
  - SECURITY: subprocesses get argument lists, never a shell string (CWE-78).
  - SECURITY: the runner uses the hardening of the P2P-AI peer-node compose file (CIS Docker 5.x): no network,
    all capabilities dropped, read-only root, non-root user, pids/memory/CPU limits.
  - SECURITY-WARNING: GATEWAY_EXEC=all and Autopilot apply to this throw-away loopback gateway only.
Dependencies: Python standard library; Go toolchain (unless --bin-dir); Docker; Node.js (workspace tool host);
cargo for the IDE core. Linux only.
"""
from __future__ import annotations

import argparse
import json
import os
import secrets
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

IDE = Path(__file__).resolve().parents[1]
GATEWAY_ID = "ide-live-e2e"
MODEL = "mock-model:1b"
RUNNER_UID, RUNNER_GID = 10002, 10001
FIXED = "def add(a, b):\n    return a + b\n"


def mock_model(p2p: Path, port: int) -> int:
    """A test-only Ollama stand-in: answers Autopilot's plan call with one task, and its work run with a fixed
    script (read_file, write_file, then a closing line). SECURITY: 127.0.0.1 only; no file or shell access."""
    sys.path.insert(0, str(p2p / "deploy"))
    sys.path.insert(0, str(p2p / "P2P-AI-Nodes" / "Python"))  # mock_ollama imports tests.mocks from there
    from http.server import ThreadingHTTPServer

    import mock_ollama as mo

    class Scripted(mo.ComposeOllamaHandler):
        def do_POST(self):
            b = self.body()
            msgs = [m for m in b.get("messages") or [] if isinstance(m, dict)]
            if self.path == "/api/chat" and b.get("tools"):
                done = sum(1 for m in msgs if m.get("role") == "tool")
                script = [("read_file", {"path": "src/calc.py"}), ("write_file", {"path": "src/calc.py", "text": FIXED})]
                if done < len(script):
                    name, args = script[done]
                    return self.ndjson([{"message": {"role": "assistant", "content": "", "tool_calls": [
                        {"function": {"name": name, "arguments": args}}]}, "done": False}, {"done": True, "eval_count": 5}])
                return self.ndjson([{"message": {"role": "assistant", "content": "Fixed add()."}, "done": False},
                                    {"done": True, "eval_count": 4}])
            if self.path == "/api/chat" and msgs and "You plan the next steps" in str(msgs[0].get("content", "")):
                plan = json.dumps({"tasks": [{"title": "Fix add in src/calc.py",
                                              "detail": "add() subtracts; make it return a + b"}]})
                return self.ndjson([{"message": {"role": "assistant", "content": plan}, "done": False},
                                    {"done": True, "eval_count": 7}])
            return super().do_POST()

    srv = ThreadingHTTPServer(("127.0.0.1", port), Scripted)
    srv.daemon_threads = True
    srv.owner = mo.Mock([MODEL])
    srv.owner.hang = 0.0
    srv.serve_forever()
    return 0


def main() -> int:
    if len(sys.argv) >= 4 and sys.argv[1] == "--mock-model":
        return mock_model(Path(sys.argv[2]), int(sys.argv[3]))
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--p2p-ai", required=True, help="path of a P2P-AI checkout")
    ap.add_argument("--bin-dir", help="folder with prebuilt `gateway` and `p2pnode` (default: go build them)")
    ap.add_argument("--image", default="p2p-ai/sandbox:dev", help="runner image (default p2p-ai/sandbox:dev)")
    ap.add_argument("--keep", action="store_true", help="keep the work folder, logs and the runner container")
    args = ap.parse_args()
    p2p = Path(args.p2p_ai).resolve()
    sys.path.insert(0, str(p2p / "scripts"))
    from shell_session_stack_e2e import Checks, call, child_env, free_port, wait_for  # noqa: E402

    for tool in ("docker", "cargo") + (() if args.bin_dir else ("go",)):
        if shutil.which(tool) is None:
            print(f"{tool} is required", file=sys.stderr)
            return 2
    node_js = shutil.which("node")
    if node_js is None:
        print("Node.js is required (workspace tool host)", file=sys.stderr)
        return 2
    # Build the example first, so its build time does not count against the mission.
    r = subprocess.run(["cargo", "build", "--quiet", "--locked", "--example", "live_missions"], cwd=IDE / "core")
    if r.returncode != 0:
        return 2
    example = IDE / "core" / "target" / "debug" / "examples" / "live_missions"

    work = Path(tempfile.mkdtemp(prefix="ide-live-e2e-"))  # SECURITY: 0700, unique (CWE-377)
    name = f"p2p-ide-live-e2e-{os.getpid()}"
    procs, logs = [], []
    c = Checks()
    try:
        if args.bin_dir:
            gw_bin, node_bin = Path(args.bin_dir) / "gateway", Path(args.bin_dir) / "p2pnode"
        else:
            gw_bin, node_bin = work / "gateway", work / "p2pnode"
            for out, cwd, pkg in ((gw_bin, p2p / "P2P-AI-Gateways" / "Go", "./cmd/gateway"),
                                  (node_bin, p2p / "P2P-AI-Nodes" / "Go", "./cmd/p2pnode")):
                r = subprocess.run(["go", "build", "-o", str(out), pkg], cwd=cwd, capture_output=True, text=True)
                if r.returncode != 0:
                    print(r.stderr[-2000:], file=sys.stderr)
                    return 2

        admin, enroll, signing = (secrets.token_hex(24) for _ in range(3))
        gw_port, node_port, ol_port, st_port = free_port(), free_port(), free_port(), free_port()
        (work / "enroll.token").write_text(enroll)
        (work / "enroll.token").chmod(0o600)

        def start(label: str, argv: list[str], env: dict) -> subprocess.Popen:
            log = open(work / f"{label}.log", "wb")
            logs.append(log)
            p = subprocess.Popen(argv, cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
            procs.append(p)
            return p

        start("model", [sys.executable, str(Path(__file__).resolve()), "--mock-model", str(p2p), str(ol_port)],
              child_env(work, {}))
        ol = f"http://127.0.0.1:{ol_port}"
        if not wait_for("scripted model", lambda: call(ol, "GET", "/api/version", timeout=2)[0] == 200, 20):
            return 2

        gw = start("gateway", [str(gw_bin)], child_env(work, {
            "GATEWAY_LISTEN": f"127.0.0.1:{gw_port}", "GATEWAY_ID": GATEWAY_ID, "GATEWAY_ADMIN_KEY": admin,
            "GATEWAY_ENROLL_TOKEN": enroll, "GATEWAY_SIGNING_KEY": signing,
            "GATEWAY_NODE_LISTEN": f"127.0.0.1:{node_port}", "GATEWAY_NODE_KEY_FILE": str(work / "gateway-node.key"),
            "GATEWAY_NODE_PUBKEY_FILE": str(work / "gateway_public_key_hex"),
            "GATEWAY_ATTACHMENT_DIR": str(work / "attachments"), "GATEWAY_WORKSPACE_DIR": str(work / "workspaces"),
            "GATEWAY_WORKSPACE_SYNC": "true", "GATEWAY_WORKSPACE_TOOLS": "true", "GATEWAY_EXEC": "all",
            "GATEWAY_AGENT_RUNS": "true", "GATEWAY_AUTOPILOT": "true", "GATEWAY_AUTOPILOT_BEST_OF_N": "1"}))
        G = f"http://127.0.0.1:{gw_port}"
        if not wait_for("gateway health", lambda: call(G, "GET", "/health", timeout=2)[0] == 200 or gw.poll() is not None, 30) \
                or gw.poll() is not None:
            print("the gateway did not start; run with --keep and read gateway.log", file=sys.stderr)
            return 2
        pub_file = work / "gateway_public_key_hex"
        pub = wait_for("gateway public key", lambda: pub_file.exists() and pub_file.read_text().strip(), 10)
        if not pub:
            return 2

        sock_dir, key_dir = work / "sock", work / "gwkey"
        sock_dir.mkdir(mode=0o770)
        key_dir.mkdir(mode=0o755)
        (key_dir / "gateway_public_key_hex").write_text(pub + "\n")
        (key_dir / "gateway_public_key_hex").chmod(0o644)
        uid, gid = (RUNNER_UID, RUNNER_GID) if os.geteuid() == 0 else (os.getuid(), os.getgid())
        if os.geteuid() == 0:
            os.chown(sock_dir, uid, gid)
        r = subprocess.run([
            "docker", "run", "-d", "--name", name, "--network", "none", "--cap-drop", "ALL",
            "--security-opt", "no-new-privileges:true", "--read-only", "--user", f"{uid}:{gid}", "--pids-limit", "256",
            "--memory", "1g", "--memory-swap", "1g", "--cpus", "1", "--ulimit", "core=0",
            "--tmpfs", f"/work:rw,exec,nosuid,nodev,size=512m,uid={uid},gid={gid},mode=0700",
            "--tmpfs", f"/tmp:rw,nosuid,nodev,noexec,size=16m,uid={uid},gid={gid},mode=0700",
            "-e", f"P2P_SANDBOX_GATEWAY_ID={GATEWAY_ID}", "-e", "P2P_SANDBOX_GATEWAY_PUBKEY_FILE=/gateway/gateway_public_key_hex",
            "-v", f"{key_dir}:/gateway:ro", "-v", f"{sock_dir}:/run/p2p-sandbox", args.image], capture_output=True, text=True)
        if r.returncode != 0:
            print(r.stderr[-1000:], file=sys.stderr)
            return 2
        sock = sock_dir / "sandbox.sock"
        if not wait_for("runner socket", lambda: sock.exists(), 30):
            return 2

        cfg = {"node_name": GATEWAY_ID, "gateway_url": f"http://127.0.0.1:{node_port}", "gateway_id": GATEWAY_ID,
               "gateway_public_key_hex": pub, "identity_key_file": str(work / "node.key"),
               "enroll_token_file": str(work / "enroll.token"), "ollama_url": ol, "heartbeat_interval_s": 1,
               "status_listen": f"127.0.0.1:{st_port}", "workspace_root": str(work / "node-workspaces"),
               "sandbox_socket": str(sock), "mcp_host": {"enabled": True, "node_path": node_js},
               "max_cpu_util_pct": 100, "max_ram_util_pct": 100, "hardware": {"cpu_cores": 4, "ram_gb": 16},
               "allowed_models": [MODEL]}
        (work / "node.json").write_text(json.dumps(cfg))
        (work / "node.json").chmod(0o600)
        start("node", [str(node_bin), "--config", str(work / "node.json")], child_env(work, {}))

        st, d, _ = call(G, "POST", "/v1/admin/keys", admin, {"name": "ide-live", "monthly_token_cap": 100000})
        key = d.get("key", "")
        if not c.check(st == 201 and bool(key), "a throw-away key on the gateway"):
            return 1
        ready = wait_for("the node serves the model",
                         lambda: any(m.get("id") == MODEL for m in call(G, "GET", "/v1/models", key)[1].get("data", [])), 60, 1)
        if not c.check(bool(ready), "the node attached and serves the model"):
            return 1

        (work / "client.key").write_text(key)
        (work / "client.key").chmod(0o600)
        out = subprocess.run([str(example), G, str(work / "client.key"), str(work), MODEL],
                             cwd=work, env=child_env(work, {}), capture_output=True, text=True, timeout=900)
        print(out.stdout, end="", flush=True)
        if out.returncode not in (0, 1) and out.stderr.strip():
            print(out.stderr[-3000:], file=sys.stderr)
        lines = out.stdout.strip().splitlines()
        try:
            sub = json.loads(lines[-1]) if lines else {}
        except ValueError:
            sub = {}
        c.passed += int(sub.get("passed", 0))
        c.failed += max(int(sub.get("failed", 1)), 1 if out.returncode != 0 else 0)
        secret = b"ide-live-do-not-send"
        leaked = any(p.is_file() and secret in p.read_bytes()
                     for base in (work / "attachments", work / "workspaces", work / "node-workspaces") if base.exists()
                     for p in base.rglob("*"))
        c.check(not leaked, "the folder's .env value is in no file the gateway or node stored")
    finally:
        for p in procs:
            p.terminate()
        for p in procs:
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
        for log in logs:
            log.close()
        if not args.keep:
            subprocess.run(["docker", "rm", "-f", name], capture_output=True)
            shutil.rmtree(work, ignore_errors=True)
        else:
            print(f"kept: {work} and container {name}", flush=True)
    print(f"SUMMARY: {c.passed} passed, {c.failed} failed", flush=True)
    return 0 if c.failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
