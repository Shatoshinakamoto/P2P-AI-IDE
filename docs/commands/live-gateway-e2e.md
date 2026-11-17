# `scripts/live_gateway_e2e.py`: live check of missions and runs against a real gateway

Development test only; not part of the app. Added 2026-11-17 for phase I4 ([features/missions.md](../features/missions.md)),
when Pheonix asked for a real-gateway test before merging PR #4. `--example live_runs` added 2026-11-17 for phase
I5b ([features/run-and-test.md](../features/run-and-test.md)).

## Purpose

The core and UI tests use fakes. This script starts the real P2P-AI parts on 127.0.0.1 (Go gateway, Go node, the
sandbox runner in Docker, a scripted test-only model) and runs the IDE core's example `core/examples/live_missions.rs`
against them. The example uses the same core functions and request shapes as the Missions view.

## Synopsis

```
python3 scripts/live_gateway_e2e.py --p2p-ai PATH [--example live_missions|live_runs] [--bin-dir DIR] [--image NAME] [--keep]
```

No root needed except what Docker needs on the machine. Linux only.

| Option | Argument | Default | What it does |
|---|---|---|---|
| `--p2p-ai` | path (required) | none | a P2P-AI checkout: its Go gateway and node sources, `deploy/mock_ollama.py`, `scripts/shell_session_stack_e2e.py` helpers |
| `--example` | `live_missions` or `live_runs` | `live_missions` | which core example runs against the stack (fixed list): missions (I4) or run and test through the sandbox tools (I5b) |
| `--bin-dir` | folder | build with `go build` | use prebuilt `gateway` and `p2pnode` binaries from this folder |
| `--image` | image name | `p2p-ai/sandbox:dev` | the sandbox runner image; build it first in the P2P-AI checkout: `docker build -f deploy/Dockerfile.sandbox -t p2p-ai/sandbox:dev .` |
| `--keep` | none | off | keep the work folder (logs, gateway and node files) and the runner container for debugging |

Environment variables: none are read. Children get an environment built from scratch (PATH and HOME only).

## What it checks

Create a project; send the folder (4 files, `.env` kept local; a second send changes nothing); the node copy is
ready; Autopilot switched on; start an Autopilot mission with a unittest check; a second mission of the same kind
gets the fixed message; approving with another digest is refused; the write calls are approved with the shown
digest; the mission finishes with its check passing in the sandbox; collect the node's changes; the fix waits in
Changes to review; nothing written before Accept; `.env` untouched; Accept writes the fix; the `.env` value is in no
file the gateway or node stored.

With `--example live_runs`: create a project; run unittest in the sandbox (stages reported; one of two tests fails
and is named; the node is named); fix the file locally and run again (the change reaches the node copy, both pass);
a command as an argument list (`;` and `$HOME` reach the program as text); no network inside the sandbox; a run past
its 10 s limit is stopped; a folder outside the root is refused before sending; the `.env` value is in no stored file.

Output: `PASS`/`FAIL` lines and `SUMMARY: n passed, n failed`. Exit codes: 0 all passed, 1 a check failed, 2 setup
error.

## Security

* **SECURITY-WARNING:** the throw-away gateway runs with `GATEWAY_EXEC=all` and `GATEWAY_AUTOPILOT=true` on loopback
  only. Never use these settings on a shared gateway.
* Secrets (admin, enroll, signing, API key) come from a CSPRNG, live in a 0700 temp folder, reach children through
  owner-only files, and are never printed (CWE-532, CWE-214, CWE-526).
* Subprocesses get argument lists, never a shell (CWE-78). The runner has no network, drops all capabilities, has a
  read-only root and runs as a non-root user.
* The scripted model is test-only, loopback-only, and has no file or shell access.

## Examples

```
# Basic: build everything from the checkout
python3 scripts/live_gateway_e2e.py --p2p-ai ../P2P-AI
# Faster reruns with prebuilt binaries
python3 scripts/live_gateway_e2e.py --p2p-ai ../P2P-AI --bin-dir /tmp/bins
# Phase I5b: run and test through the gateway sandbox
python3 scripts/live_gateway_e2e.py --p2p-ai ../P2P-AI --example live_runs
# Keep the logs after a failure
python3 scripts/live_gateway_e2e.py --p2p-ai ../P2P-AI --bin-dir /tmp/bins --keep
```

If Docker Hub answers 429 while building the image, start dockerd with `--registry-mirror https://mirror.gcr.io`.

## Test evidence

* [Live run at bfae51f: 19 passed, 0 failed](../../HistoryTest/runs/2026-11-17_085225Z_live-gateway_bfae51f/RUN.md)
