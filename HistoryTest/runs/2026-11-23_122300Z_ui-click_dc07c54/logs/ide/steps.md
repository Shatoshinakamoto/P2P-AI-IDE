# P2P-AI IDE desktop click-through

* Repo: Shatoshinakamoto/P2P-AI-IDE, `main` at `dc07c54892d791b3739a3fa897566a292ca8d58a`
  ("Merge pull request #28"), fresh clone with `core.autocrlf=false`.
* Build: `cd app && npm ci --ignore-scripts && npx tauri build --debug --no-bundle` (debug exe), exit 0, 2 min 51 s.
  `npm ci`: found 0 vulnerabilities. Log: `build.log`.
* Driven through WebView2 CDP (port 9301, set for the app process only) with Playwright `connectOverCDP`.
  The native folder picker was filled through Win32 messages (WM_SETTEXT / BM_CLICK); no screenshot of that
  dialog is kept because it shows account and drive names.
* Console: `console.log` (logger attached 5 s after start; one `click-probe` warning is my own logger check).
  The IDE logged **no** console errors, warnings, page errors or failed requests. App stderr: `app-stderr.log`.
* Times: `../times.txt` (UTC).

| # | Step | Expected | Result | Evidence |
|---|---|---|---|---|
| 1 | Launch debug exe | Window shows the editor shell, not blank | **pass**: activity bar, Explorer, editor area, status bar | 01-launch.png |
| 2 | Open folder `ide-click-ws` (OS dialog) | Explorer lists `hello.py` | **pass**: title `ide-click-ws`, status "Opened ide-click-ws" | 02-open-folder-click.png, 03-folder-open.png |
| 3 | Open `hello.py` | File in Monaco with Python highlighting | **pass**: 8 lines, language `python` | 04-hello-open.png |
| 4 | Gateways side bar | "P2P-AI accounts" panel with address field and Sign in | **pass**: field placeholder `https://accounts.example.org`, **Sign in** button | 05-gateways.png |
| 5 | Add gateway "Ollama dev" `http://127.0.0.1:11434`, throwaway key | Saved, INSECURE — DEV ONLY badge, key in keychain | **pass** | 06-add-gateway-form.png, 07-gateway-added.png |
| 6 | Check gateway | latency and model count | **pass**: "2436 ms, 56 models" | 08-gateway-check.png |
| 7 | Agent chat, `qwen2.5-coder:7b`, Tools on (default) | Answer | **fail (model/tool-call)**: reply is the raw text `{"name": "read_file", "arguments": {"path": "hello.py"}}`; the IDE shows it as the answer and runs no tool (bug 1) | 09-chat-view.png, 10-chat-answer.png |
| 8 | Agent chat, `qwen2.5:14b`, Tools on | Tool use + answer | **pass**: `read_file hello.py` tool step, then a correct answer (49 s) | 11-chat-answer-14b-tools.png |
| 9 | Agent chat, `qwen2.5-coder:7b`, Tools off | Answer | **pass**: "5" to "what is 2 + 3?" (15.7 s) | 12-tools-off.png, 13-chat-answer-coder7b-tools-off.png |
| 10 | Extensions view / Node extensions panel | Opens | **pass**: "Node extensions (1)", trust button, Electron compatibility switch, installed list, colour theme | 14-extensions.png |
| 11 | Settings panel | Opens | **n/a**: the IDE has no Settings view or command (activity bar: Explorer, Search, Source control, Run and test, Agent chat, Missions, Gateways, Extensions); settings live in the Extensions view (theme) and Gateways | — |
| 12 | Search, Source control, Run and test, Missions, Explorer views | Open without errors | **pass** (Source control: "this folder is not the top of a git repository", correct for the test folder) | 15-search.png … 19-explorer.png |
| 13 | Remove "Ollama dev" (two clicks) | Gateway and keychain entry gone | **pass**: second click is "Confirm"; `gateways.json` back to `[]`; `cmdkey /list` no longer lists the `org.p2p-ai.ide.gateway` entry | 20-gateway-remove-confirm.png, 21-gateway-removed.png |
| 14 | Close window | Process exits | **pass**: no `p2p-ide` or its WebView2 left | — |

## Bugs and notes

1. **Text tool call shown as the answer (qwen2.5-coder:7b, Tools on).** The model returns its tool call as plain
   content (`{"name": "read_file", "arguments": {"path": "hello.py"}}`) instead of `tool_calls`; the IDE ends the turn
   and shows that JSON as the reply. Known quirk of this model through Ollama's OpenAI API, but the IDE could either
   parse a lone JSON tool call in content or tell the person the model does not support tools and suggest Tools off.
2. **Run and test: stale hint.** With a gateway saved, the "Where to run" option still reads "Gateway sandbox ... (add a
   gateway in the Gateways view first)". (The saved gateway is a plain Ollama, not a P2P-AI gateway, so the option is
   rightly unusable, but the hint is wrong; it should say the gateway has no linked project / is not a P2P-AI gateway.)
3. **Missions offers Ollama as a project host.** "Link this folder to a project on Ollama dev" is offered for a plain
   Ollama endpoint that cannot host projects. Minor.
4. **No Settings panel** exists to check (step 11); not a bug if intended, noted because the checklist asked for it.
5. Setting `APPDATA`/`LOCALAPPDATA` for the process did **not** isolate app data (Tauri resolves the known folder),
   so the test used the real per-user `org.p2p-ai.ide` folder; it had no gateways before and has none after.
