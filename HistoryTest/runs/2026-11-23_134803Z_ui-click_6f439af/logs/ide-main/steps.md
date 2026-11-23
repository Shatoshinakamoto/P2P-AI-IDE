# Round 4, extra (owner's request): click-through on a clean, fresh clone of P2P-AI IDE main

* `git clone --branch main https://github.com/Shatoshinakamoto/P2P-AI-IDE D:\p2p-ai-ci\ide-main-r4`: HEAD
  `6f439afdc7f7dad710fffcb3e8b4ebc6e90794ef` ("Merge pull request #29 …"). PR #30 is **not** in main.
* `npm ci` (0 vulnerabilities), `npx tauri build --no-bundle` (3 min 21 s), exit 0: `02-npm-ci.log`, `03-build-release.log`.
* Same steps and scripts as Part B, CDP port 9305, window off-screen.

| # | Step | Result | Evidence |
|---|---|---|---|
| 1 | Launch | pass | 01-launch.png |
| 2 | Open folder ide-click-ws, hello.py | pass | 02-folder-open.png |
| 3 | Add gateway Ollama dev (throwaway key) | pass, key in keychain | 03-gateway-added.png |
| 4 | Agent chat, qwen2.5-coder:7b, Tools on, "read hello.py and tell me what it prints" | **fail (known bug, fixed in PR #30)**: the reply is only the raw text `{"name": "read_file", "arguments": {"path": "hello.py"}}`; no tool step ran, no answer | 04-chat-coder7b-tools-on.png |
| 5 | New chat, same prompt | same result | 05-chat-retry-new-chat.png |
| 6 | Remove gateway | list empty, `gateways.json` = `{"gateways": []}`, 0 keychain matches | — |
| 7 | Close | 0 p2p-ide, 0 logger processes | times.txt |

hello.py unchanged. The clone `D:\p2p-ai-ci\ide-main-r4` is left in place (clean tree, build output only).