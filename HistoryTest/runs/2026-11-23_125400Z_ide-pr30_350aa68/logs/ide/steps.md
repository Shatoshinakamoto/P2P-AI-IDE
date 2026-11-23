# Round 3, Part A: P2P-AI IDE PR #30 (branch claude/project-thread-w0i40c)

* Checkout: `ide-click2`, `git fetch origin claude/project-thread-w0i40c`, `git checkout -B pr30 FETCH_HEAD`;
  HEAD `350aa68004e6e66fa06b250c3f89cece2e26f02f` ("Fix three click-through bugs ..."), parent ec50694. Clean tree. `01-checkout.log`.
* Times (UTC) are at the top/bottom of every log; app launch/close in `times.txt`.

## Tests and build

| Log | Command (folder) | Exit | Result |
|---|---|---|---|
| 02-cargo-test-core.log | `cargo test` (core) | 0 | 140 passed, 0 failed, 4 ignored (7 test binaries) |
| 03-cargo-check-src-tauri.log | `cargo check` (app\src-tauri) | 0 | Finished |
| 04-npm-ci.log | `npm ci` (app) | 0 | installed |
| 05-npm-run-check.log | `npm run check` (tsc --noEmit) | 0 | 0 errors |
| 06-npm-test.log | `npm test` | 0 | 8 passed, 0 failed |
| 07-npm-ui-smoke.log | `npm run ui-smoke` | **1** | environment: `Cannot find module 'playwright'` (the script expects a global playwright; not a project dependency) |
| 07b-npm-ui-smoke-nodepath.log | same, NODE_PATH = installed @playwright/mcp's playwright 1.65.0-alpha | **1** | environment: wants Chromium headless shell 1252, only 1248 is on the PC (no download made) |
| 07c-npm-ui-smoke-shim.log | same, NODE_PATH = local shim (`click-drv\shim`): that playwright-core + `executablePath` of the existing headless shell 1248 | 0 | **135 passed, 0 failed**, no CSP violations, no page errors (dist/ from the release build) |
| 08-build-release.log | `npx tauri build --no-bundle` (app) | 0 | `p2p-ide.exe` built (3 min 53 s) |

## Click-through (release exe, WebView2 CDP port 9304, window moved off-screen to -32000,-32000 so it never showed on the desktop)

| # | Step | Expected | Result | Evidence |
|---|---|---|---|---|
| 1 | Launch | Editor shell | pass | 01-launch.png |
| 2 | Open folder `ide-click-ws` **first** | Explorer lists hello.py | pass | 02-…, 03-folder-open.png |
| 3 | Open hello.py | Monaco, Python | pass | 04-hello-open.png |
| 4 | Gateways > Add gateway: "Ollama dev", `http://127.0.0.1:11434`, throwaway key | Saved, INSECURE — DEV ONLY | pass, "key in keychain" | 05-…, 06-…, 07-gateway-added.png |
| 5 | Check | latency + models | pass: "2635 ms, 56 models" | 08-gateway-check.png |
| **b** | Run and test (folder opened before the gateway was added) | no "add a gateway in the Gateways view first" | **pass**: "Gateway sandbox (no network, changes not kept) (link this folder to a gateway project first)" + "Link the folder in Missions" | 09-run-and-test.png |
| **c** | Missions on Ollama dev | "model endpoint without projects", no "Create project and link" | **pass**: "Missions run on a P2P-AI gateway project. Ollama dev is a model endpoint without projects (for example Ollama): use Agent chat with it, or switch to a P2P-AI gateway." Only button: "Open Gateways" | 10-missions.png |
| **a** | Agent chat, qwen2.5-coder:7b, Tools on, "read hello.py and tell me what it prints" | read_file tool step, not a JSON reply | **pass**: tool step `read_file hello.py` ran, answer "The code in `hello.py` prints: Hello, world!" (9.6 s). Cosmetic: the model's raw `{"name": "read_file", "arguments": {"path": "hello.py"}}` text stays visible above the tool step | 11-chat-view.png, 12-chat-coder7b-tools-on.png |
| 6 | Remove gateway (two clicks: "Click again to remove Ollama dev") | Gone | pass: list empty, `gateways.json` = `{"gateways": []}`, 0 keychain entries matching p2p/ollama | 13-…, 14-gateway-removed.png |
| 7 | Close window, stop logger | Nothing left | pass: 0 p2p-ide, 0 driver processes | — |

Console (`console.log`): no errors, warnings, page errors or failed requests. App stdout (`app-stderr.log`): 4 audit lines
(workspace opened, gateway added_manual, agent turn, gateway removed).

## Notes

1. Small inconsistency (not a regression): with a plain Ollama gateway, Run and test still says "Link the folder in
   Missions", while Missions (correctly) says this gateway has no projects. Run and test could say the same for a
   model endpoint.
2. The app ignores the APPDATA/LOCALAPPDATA override (it uses the Known Folder API), so the gateway list was written
   to the real `<home>\AppData\Roaming\org.p2p-ai.ide\gateways.json`; it is back to `{"gateways": []}`.
3. Cosmetic in (a): the streamed tool-call JSON is not hidden once it is turned into a tool step.
