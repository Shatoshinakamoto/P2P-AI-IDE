# P2P-AI IDE desktop click-through, run 2 (latest main)

* Requested 2026-11-23 12:32Z: rerun the IDE click-through on a clean, most recent pull of `main`.
* Repo: Shatoshinakamoto/P2P-AI-IDE, **fresh clone** (`core.autocrlf=false`) into `ide-click2`, `main` at
  `ec50694c3f35f7f21a702514f424e80965a478b0` ("Add NEXUS Engine support and finish account/gateway IDE fixes",
  2026-11-23T12:31:02Z). Clean tree after clone.
* Build: `cd app && npm ci --ignore-scripts && npx tauri build --debug --no-bundle`, exit 0, 2 min 24 s;
  `npm ci`: found 0 vulnerabilities. Logs: `npm-ci.log`, `build.log`.
* Same method as run 1 (`../ide/steps.md`): WebView2 CDP on port 9303 for the app process only, Playwright
  `connectOverCDP`; folder picker filled by WM_SETTEXT/BM_CLICK, no dialog screenshot kept (it shows names).
* Console (`console.log`): **no** console errors, warnings, page errors or failed requests. App stderr: `app-stderr.log`.

| # | Step | Expected | Result | Evidence |
|---|---|---|---|---|
| 1 | Launch debug exe | Editor shell, not blank | **pass** | 01-launch.png |
| 2 | Open folder `ide-click-ws` | Explorer lists `hello.py` | **pass** | 02-open-folder-click.png, 03-folder-open.png |
| 3 | Open `hello.py` | Monaco, Python | **pass** | 04-hello-open.png |
| 4 | Gateways: P2P-AI accounts panel | Address field + Sign in | **pass**; new "NEXUS Engine on this computer" card (Proxy :11435, Runtime :11436 "not detected", Connect buttons disabled, Custom local port) | 05-gateways.png |
| 5 | Add "Ollama dev" `http://127.0.0.1:11434`, throwaway key | Saved, INSECURE — DEV ONLY | **pass** | 06-add-gateway-form.png, 07-gateway-added.png |
| 6 | Check | latency + models | **pass**: "2674 ms, 56 models" | 08-gateway-check.png |
| 7 | Chat `qwen2.5-coder:7b`, Tools on | Answer | **fail (same as run 1)**: raw `{"name": "read_file", "arguments": {"path": "hello.py"}}` shown as the answer, no tool run | 10-…, 10b-chat-coder7b-tools-on.png |
| 8 | Chat `qwen2.5:14b`, Tools on | Tool + answer | **pass**: `read_file hello.py` then correct answer (63 s); the model also printed a stray Thai sentence before the tool step (model output, not an IDE fault) | 11-chat-14b-tools-on.png |
| 9 | Chat `qwen2.5-coder:7b`, Tools off | Answer | **pass**: "2 + 3 is 5." (15.7 s) | 12-tools-off.png, 13-chat-coder7b-tools-off.png |
| 10 | Extensions / Node extensions | Opens | **pass** | 14-extensions.png |
| 11 | Settings panel | Opens | **n/a**: no Settings view exists in the IDE | — |
| 12 | Search, Source control, Run and test, Missions, Explorer | Open | **pass** | 15-… to 19-….png |
| 13 | Remove gateway (two clicks) | Gone from list and keychain | **pass**: `gateways.json` `[]`, 0 `org.p2p-ai.ide` keychain entries | 20-…, 21-gateway-removed.png |
| 14 | Close window | Exits | **pass**, no process left | — |

## Bugs still present on ec50694

1. qwen2.5-coder:7b with Tools on: a tool call returned as text is shown as the reply (no tool run, no hint).
2. Run and test: "Gateway sandbox … (add a gateway in the Gateways view first)" while a gateway is saved.
3. Missions offers "Link this folder to a project on Ollama dev" for a plain Ollama endpoint.
4. "TandemForge on this computer" says TandemForge is not running while a TandemForge window was open (it was on a
   non-default backend port 8766 for this test with its isolated database, and no app was paired in its Settings >
   Connected apps, so this is probably expected; noted, not counted as a bug).
