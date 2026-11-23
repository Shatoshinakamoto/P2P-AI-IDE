# `scripts/live_nexus_e2e.py`: live check against real NEXUS Proxy and NEXUS Runtime

Development test only; not part of the shipped app. Verifies [features/nexus-engine.md](../features/nexus-engine.md) end to end against real compiled `nexus-proxy` and `nexus-runtime` binaries on `127.0.0.1`.

## Purpose

Unit tests in `core/src/nexus.rs` and `app/tests/ui-smoke.mjs` run against synthetic HTTP fixtures and fake IPC bridges. This runner builds and launches real `nexus-proxy` and `nexus-runtime` processes from a `nexus-engine` checkout and runs `core/tests/nexus_engine.rs` against them in two passes:

1. **Mock-upstream Proxy + Runtime pass**: tests `nexus::probe_endpoint` (health, `airlock_threshold`, `simd`), keyless `GatewayStore::save_nexus`, `Client::models()`, and streamed SSE `/v1/chat/completions` against `nexus-proxy --mock-ollama` and `nexus-runtime serve`.
2. **Live tool-forwarding Proxy pass (`NEXUS_AGENT_TOOLS=1`)**: points real `nexus-proxy` at a local loopback scripted OpenAI server (`--ollama-url http://127.0.0.1:<port>`) and executes a multi-step `agent::run_turn` (`read_file` -> `edit_file` -> pending review without writing disk), while verifying local `.env` secrets never appear on the wire.

## Synopsis

```sh
python3 scripts/live_nexus_e2e.py --nexus-engine ../nexus-engine [--keep]
```

| Option | Argument | Default | Purpose |
|---|---|---|---|
| `--nexus-engine` | path (required) | none | Path to a local `nexus-engine` checkout (`Cargo.toml` root) |
| `--keep` | none | off | Keep the temporary 0700 work directory and daemon logs after exit |

Or run `cargo test --locked --test nexus_engine -- --ignored --nocapture` inside `core/` directly against already running `nexus-proxy` (`NEXUS_PROXY_URL`, default `http://127.0.0.1:11435`) and `nexus-runtime` (`NEXUS_RUNTIME_URL`, default `http://127.0.0.1:11436`) daemons.

## Security

* All servers and test targets bind `127.0.0.1` only (`NIST SC-7`, `CWE-918`).
* Child binaries are spawned with fixed argument arrays (`shell=False`, `CWE-78`).
* Temporary work dirs use mode `0700` and are deleted automatically in `finally:` unless `--keep` is passed.

## Test evidence

* [Live E2E run against real `nexus-proxy` (mock + live tools) & `nexus-runtime`](../../HistoryTest/runs/2026-11-23_114348Z_live-nexus_dc07c54/RUN.md)
