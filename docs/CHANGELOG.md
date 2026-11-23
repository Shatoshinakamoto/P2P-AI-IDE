# Changelog

All notable changes to the P2P-AI IDE. Format: [Keep a Changelog](https://keepachangelog.com), newest first.

## 2026-11-23 - NEXUS Engine support and account/gateway UX fixes

### Added
- **NEXUS Engine on this computer** ([features/nexus-engine.md](features/nexus-engine.md)): discover and connect local **NEXUS Proxy** (`127.0.0.1:11435`, Agent chat with tools + Context Packer + Data Airlock) and **NEXUS Runtime** (`127.0.0.1:11436`, native `.nexus` chat) without an API key or the `INSECURE — DEV ONLY` label.
- Core module [`core/src/nexus.rs`](../core/src/nexus.rs), `Kind::Nexus` and `GatewayStore::save_nexus` in [`core/src/gateways.rs`](../core/src/gateways.rs), keyless loopback-only `Client::new` in [`core/src/client.rs`](../core/src/client.rs), Tauri commands `nexus_status` and `nexus_connect` in [`app/src-tauri/src/net.rs`](../app/src-tauri/src/net.rs), UI component [`NexusConnect.tsx`](../app/src/components/NexusConnect.tsx), and VS Code extension command **P2P-AI: Connect NEXUS Engine** (`p2pAi.connectNexus`, [`vscode-extension/src/nexus.ts`](../vscode-extension/src/nexus.ts)).
- Live E2E test suite [`core/tests/nexus_engine.rs`](../core/tests/nexus_engine.rs) and runner [`scripts/live_nexus_e2e.py`](../scripts/live_nexus_e2e.py) ([commands/live-nexus-e2e.md](commands/live-nexus-e2e.md)).

### Fixed
- **Account-linked gateways in Chat & Missions**: `ChatView.tsx` and `MissionsView.tsx` previously gated on `gateway.has_key`, blocking gateways linked to a P2P-AI account (`gateway.account !== null`) when no API key is saved; both now accept account-linked gateways and load models/projects without requesting an API key.
- **Missions view for local engines & Status bar wording**: Missions displays clear local-engine guidance for `TandemForge` and `NEXUS Engine` instead of failing on `/v1/projects`, and `StatusBar.tsx` / `docs/user-guide.md` no longer carry stale Phase I3 / Node-host placeholder wording.

### Security
- See SECURITY-LOG 2026-11-23 (NEXUS Engine keyless loopback bounds).

### Tests
- Core [`2026-11-23_114345Z_core_dc07c54`](../HistoryTest/runs/2026-11-23_114345Z_core_dc07c54/RUN.md), Live NEXUS E2E [`2026-11-23_114348Z_live-nexus_dc07c54`](../HistoryTest/runs/2026-11-23_114348Z_live-nexus_dc07c54/RUN.md), Shell [`2026-11-23_114353Z_shell_dc07c54`](../HistoryTest/runs/2026-11-23_114353Z_shell_dc07c54/RUN.md), UI smoke [`2026-11-23_114250Z_ui_dc07c54`](../HistoryTest/runs/2026-11-23_114250Z_ui_dc07c54/RUN.md), App unit [`2026-11-23_114425Z_app-unit_dc07c54`](../HistoryTest/runs/2026-11-23_114425Z_app-unit_dc07c54/RUN.md), VS Code ext [`2026-11-23_114356Z_vscode-ext-unit_dc07c54`](../HistoryTest/runs/2026-11-23_114356Z_vscode-ext-unit_dc07c54/RUN.md), Security scan [`2026-11-23_114426Z_security-scan_dc07c54`](../HistoryTest/runs/2026-11-23_114426Z_security-scan_dc07c54/RUN.md).

## 2026-11-23 - Agent commits in the Source Control view

### Added
- **From the agent** in the Source Control view ([features/agent-commits.md](features/agent-commits.md)): commits an agent asked for with the gateway's `git_commit` mode `local`. You see the message and the diff, then click Commit (applied and committed in the open folder with your own git identity) or Reject; the gateway is told the outcome once.
- Core `scm::check_patch` and `Repo::apply_checkpoint`; `projects::checkpoint_view`; shell `agent_commits.rs` with four commands; view `AgentCommits.tsx`.

### Security
- See SECURITY-LOG 2026-11-23.

### Tests
- First cloud try of the core tests, 2 failures fixed: [2026-11-23_040711Z_ide-core_d834367](../HistoryTest/runs/2026-11-23_040711Z_ide-core_d834367/RUN.md). PC runs: see the feature page.
## 2026-11-23 - VS Code extension: sign in with a P2P-AI account (A3)

### Added
- Extension commands **P2P-AI: Sign in with a P2P-AI account**, **Change account password**, **Sign out of a P2P-AI account**, **Use a P2P-AI account for a gateway** and **Use a key for a gateway**. The same sign-in as the IDE (system browser, PKCE S256, one-time loopback address, `iss` check), with the extension's own client id `p2p-ai-ide`. Extension version 0.4.0. Page: [features/account-sign-in.md](features/account-sign-in.md#vs-code-and-vscodium-extension).
- New `vscode-extension/src/accounts.ts` (a port of `core/src/accounts.rs` with the same messages) and `vscode-extension/tests/accounts.test.ts` (9 tests against a fake account service and a fake browser).
- `.gitattributes`: HistoryTest files stay LF on every checkout, so Windows clones keep the manifest chain intact.

### Changed
- A saved gateway can carry an `account` link. Chat and Missions ask for a fresh account token before each request; the key field is not used while linked.

Test evidence (Windows PC): HistoryTest `2026-11-23_035759Z_vscode-ext-check_9243fa4` (tsc: 0 errors), `2026-11-23_035800Z_vscode-ext-unit_9243fa4` (44 of 44, including the 9 accounts tests). `2026-11-23_035621Z` and `2026-11-23_035628Z` are kept but marked invalid: the clone used `core.autocrlf=true`, which changed the hashed bytes.

## 2026-11-23 - Accounts: sign in to gateways with a P2P-AI account (ADR-030 phase A3)

### Added
- **P2P-AI accounts** panel in the Gateways side bar: sign in at an account service in the system browser (OAuth for native apps with PKCE S256 and a one-time loopback address, RFC 8252 and RFC 7636), see who is signed in, replace an operator-made one-time password, and sign out (the session at the account service ends too). Page: [features/account-sign-in.md](features/account-sign-in.md).
- Each gateway card can **Use account** (account service plus the gateway's account id) instead of a pasted key, and **Use key instead** to go back. Gateway tokens are fetched, cached in memory and renewed by the app; nothing changes for chat, runs or missions.
- New core module `core/src/accounts.rs`; shell commands `accounts_list`, `account_sign_in`, `account_sign_in_cancel`, `account_sign_out`, `account_change_password` and `gateway_set_account` (`app/src-tauri/src/account.rs`).
- `core/examples/live_accounts.rs` and `scripts/live_accounts_e2e.mjs`: the sign-in checked against a real account service and a real Go gateway in Chromium ([commands/live-accounts-e2e.md](commands/live-accounts-e2e.md)).

### Changed
- `gateways.json` entries can carry an `account` link; `accounts.json` lists remembered account services ([files/app-data.md](files/app-data.md)). The OS keychain has a second service, `org.p2p-ai.ide.account`, for refresh tokens.

Test evidence: HistoryTest `2026-11-23_031826Z_core_ffb62c3` (core 134 passed), `2026-11-23_031839Z_ui_ffb62c3` (UI smoke 126 of 126), `2026-11-23_031903Z_live-accounts_ffb62c3` (real account service and gateway: 19 of 19), `2026-11-23_032814Z_security-scan_ffb62c3` (cargo audit, npm audit, token search: pass).

## 2026-11-22 - I6d checked in the Windows desktop app

### Changed
- Docs and roadmap: I6d is now checked in the Windows desktop app on the PC. The switch is off by default, the first start downloads the pinned Electron 43.8.3, `electron.exe` runs the server and extension host, and Code Spell Checker works ([run](../HistoryTest/runs/2026-11-22_200619Z_desktop-node-ext-electron_cc16e75/RUN.md)). No code changes.

## 2026-11-22 - Phase I6d: Electron compatibility host for Node extensions

### Added
- An optional **Electron compatibility host**, off by default. When it is on, the Node extension host runs our pinned server with Electron 43.8.3 (the one VS Code 1.145.2 ships) in Node mode, so extensions with native modules built for VS Code's Electron load. Electron is downloaded once from its GitHub release on the first start after the person agrees, and checked against a pinned SHA-256. The Node extensions panel has the switch and lists enabled extensions whose native modules are built for one runtime. New core module `core/src/electron_host.rs`, shell command `node_host_electron`. Page: [features/node-extensions-design.md](features/node-extensions-design.md#11-electron-compatibility-host-i6d).
- `app/tests/electron-host-e2e.sh` and the `abi-probe` test fixture ([commands/electron-host-e2e.md](commands/electron-host-e2e.md)). `core/examples/node_host_serve` takes `--electron-zip` and `--electron-sha256`.

### Fixed
- `THIRD-PARTY-NOTICES.txt` still listed url 2.5.8 and form_urlencoded 1.2.2 after the RUSTSEC-2026-0592 bump; regenerated.

Test evidence: HistoryTest `2026-11-22_121508Z_electron-pin_01bc30e` (pins match), `2026-11-22_121508Z_core_01bc30e` (core 116 passed), `2026-11-22_121522Z_ui_01bc30e` (UI smoke 118 of 118), `2026-11-22_121549Z_node-host-electron_01bc30e` (real server under Electron: the probe activates), `2026-11-22_121624Z_node-host-electron_01bc30e` (plain Node control: fails as expected), `2026-11-22_121653Z_security-scan_01bc30e` (fail: stale notices) and `2026-11-22_121745Z_security-scan_882c782` (pass).

## 2026-11-22 - Side track X3: missions in the VS Code extension

### Added
- A **Missions** view in the extension: link the open folder to a project on the gateway, send the folder, switch Autopilot and Auto Improve, start, pause, resume and stop missions, approve or deny their commands, triage Auto Improve proposals, and collect the work into the chat's **Changes to review** list (accepted per file). Same rules as the IDE's phase I4. Extension version 0.3.0. Page: [features/vscode-extension.md](features/vscode-extension.md).
- `core/tests/vectors/missions.json`: shared mission test cases (path grammar, ids, encoding, upload names, error messages, push and pull plans, view shapes, acceptance checks), run by `core/src/projects_vectors.rs` and the extension's `tests/missions.test.ts`.
- `vscode-extension/tests/ui/missions-smoke.cjs`: draws the Missions view in Chromium and checks what it posts.

### Changed
- The extension keeps its settings (gateways without keys, active gateway, models, Tools switch, links) in `state.json` in its global storage folder. Old values move over once.

### Fixed
- On VS Code 1.147 a saved setting (a removed gateway, a new link) sometimes came back or vanished a moment later, because `globalState` writes were undone. The settings file fixes it.

Test evidence: HistoryTest `2026-11-22_114442Z_core_ae1f4aa` (core with the shared mission cases: pass), `2026-11-22_115617Z_vscode-ext-unit_0c3ceb5` (35 of 35), `2026-11-22_115617Z_vscode-ext-integration_0c3ceb5` (real VS Code 1.147.0: pass), `2026-11-22_115715Z_vscode-ext-integration_0c3ceb5` (1.95.0: pass), `2026-11-22_114505Z_vscode-ext-ui_ae1f4aa` (Missions view: pass), `2026-11-22_115718Z_vscode-ext-audit_0c3ceb5` (npm audit: 0), `2026-11-22_115718Z_vscode-ext-package_0c3ceb5` (.vsix 0.3.0). The failed run `2026-11-22_114456Z_vscode-ext-integration_ae1f4aa` found the settings race.

## 2026-11-22 - Side track X2: agent tools with diff review in the VS Code extension

### Added
- The extension's chat has a **Tools** switch (on by default). With a trusted local folder open, the model can list, read and search the folder and propose edits and new files, with the same five tools, rules and limits as the IDE's phase I3. Each tool step shows in the chat. Proposals are listed under **Changes to review**: **Review** opens VS Code's read-only diff, and **Accept** (also in the diff's title bar) writes the file with a version check. **Reject** drops the change. New commands: **P2P-AI: Accept Change**, **P2P-AI: Reject Change**. Extension version 0.2.0. Page: [features/vscode-extension.md](features/vscode-extension.md).
- `core/tests/vectors/agent_tools.json`: shared agent-tool test cases (tool definitions and system prompt byte for byte, path rules, deny list, path normalising, edits with Windows line endings, line ranges, line counts, argument errors). The IDE runs them in `core/src/agent_vectors.rs` and the extension in `tests/agent.test.ts`.
- `vscode-extension/tests/ui/view-smoke.cjs`: draws the chat view in Chromium with a sample state and checks what it posts.

### Changed
- The extension declares limited support for Restricted Mode and virtual workspaces: chat works there, tools are off.

Test evidence: HistoryTest `2026-11-22_113321Z_core_67b1744` (core 104 + 7 passed), `2026-11-22_113334Z_vscode-ext-unit_67b1744` (extension 25 of 25), `2026-11-22_113346Z_vscode-ext-integration_67b1744` (real VS Code 1.147.0: pass), `2026-11-22_113352Z_vscode-ext-integration_67b1744` (real VS Code 1.95.0: pass), `2026-11-22_113429Z_vscode-ext-ui_739ab5c` (chat view: pass), `2026-11-22_113405Z_vscode-ext-audit_67b1744` (npm audit: 0), `2026-11-22_113405Z_vscode-ext-package_67b1744` (.vsix 0.2.0 built).

## 2026-11-22 - Side track X1: P2P-AI extension for VS Code and VSCodium

### Added
- `vscode-extension/`: a P2P-AI view with a streamed chat (Stop, New), plus the commands Add, Pick and Remove Gateway, Pick Model and Set Gateway Key. Keys are kept in VS Code's secret storage, and each gateway can have its own CA. It is built as `p2p-ai.vsix` with no run-time dependencies. Pages: [features/vscode-extension.md](features/vscode-extension.md), [commands/vscode-extension.md](commands/vscode-extension.md).
- `core/tests/vectors/gateway_client.json`: shared test cases that the IDE's Rust gateway client and the extension's TypeScript client both run, covering SSE lines, model lists, HTTP status, model ids, URLs, names and keys.

### Fixed
- The gateway URL check dropped text after an IPv6 `]` (for example `http://[::1]x`) instead of refusing it. The shared test cases found it.

Test evidence: HistoryTest `2026-11-22_112152Z_core_a704618` (core 99 + 5 passed, with the shared cases), `2026-11-22_112214Z_vscode-ext-unit_a704618` (extension 14 of 14), `2026-11-22_112214Z_vscode-ext-integration_a704618` (real VS Code 1.147.0: pass), `2026-11-22_112220Z_vscode-ext-integration_a704618` (real VS Code 1.95.0: pass), `2026-11-22_112222Z_vscode-ext-audit_a704618` (npm audit: 0), `2026-11-22_112223Z_vscode-ext-package_a704618` (.vsix built).

## 2026-11-22 - Phase I6c-4: our own Node extension host server

### Added
- Pheonix chose "Build our own server": `scripts/build_node_host.py` builds the Node extension host server from VS Code's MIT source at the editor library's exact version (1.145.2), with the Copilot extension and runtime and the marketplace debugger extensions left out. Linux x64 build: 93 MB. Page: [commands/build-node-host.md](commands/build-node-host.md).
- `core/examples/node_host_serve.rs`: installs a local server build through the core's own hash check, unpack, trust and start, for testing before it is published.
- UI smoke test option `NODE_HOST_REAL`: forwards the editor's connection to such a real server and checks in the server's own logs that it got the workspace and started extensions.

### Fixed
- Security: `url` 2.5.8 -> 2.5.9 in the desktop app (RUSTSEC-2026-0592, a crash on nested `blob:` URLs).
- Extensions could not save their state (`globalState`): the editor now registers the extension storage service (in memory; state lasts until the window reloads). Found with the real host: Git's first write failed.

### Changed
- Pins: the Node extension host is now our own build, published on the IDE repo's pre-release `node-host-1.145.2`. The repo was made public on 2026-11-22 so the app can download without signing in. Linux x64 is pinned (`SERVER_BUILD` 1.145.2, `VSCODE_VERSION` 1.145.2). The VSCodium 1.140 pins are removed, because the editor refused them anyway. The download size text now says about 90 MB.

- Windows x64 is pinned too (built on the PC, 86.6 MB). `scripts/build_node_host.py` now finds the Windows SDK's `signtool.exe`, which VS Code's Windows packaging step needs.

### Not done yet
- macOS and Linux arm64 have no builder, so they show Node extensions as not available. Checked on the PC: Code Spell Checker runs in the Windows desktop app with our server.

### Tests
- Linux x64 server build [run](../HistoryTest/runs/2026-11-22_053759Z_node-host-build_4096b2e/RUN.md); the real editor with that server, 116 of 116 [run](../HistoryTest/runs/2026-11-22_053720Z_ui-node-host-real_832a0a7/RUN.md) (an earlier run [is corrected by a note](../HistoryTest/runs/2026-11-22_053624Z_ui-node-host-real_4096b2e/NOTES.md)).

## 2026-11-22 - Phase I6c-3: Node host PC fixes and version check

### Fixed
- Node extensions were staged from the wrong folder, so the server listed none of them; the store's real layout is now copied.
- On Windows the Node host's `node.exe` (and container engine calls) opened a visible console window; they now start without one.
- Windows folder paths reached the editor as `\\?\D:\...`, which broke every file request of the Node host; the shell now sends the plain path.

### Changed
- Network folders (`\\server\share`) are refused for Node extensions with a clear message (`folder_not_local`).
- The shell reports the pinned server's VS Code version; the editor offers and connects the Node host only when it matches its own VS Code minor version, and the panel says why otherwise. Found on the PC: the editor library is VS Code 1.145.2 and the pinned VSCodium server 1.140, whose messages are numbered differently, so no extension could start. Node extensions therefore show as unavailable until a matching server is pinned (choice pending).
- The design page now says the editor library is built on VS Code 1.145.2 (it said 1.140); download size is about 85 MB.

### Tests
- PC click-through in the desktop app, partial (3 bugs fixed, version check verified, no Node extension ran) [run](../HistoryTest/runs/2026-11-22_042952Z_node-host_c727219/RUN.md); core unit tests 95 passed [run](../HistoryTest/runs/2026-11-22_042549Z_core_c727219/RUN.md); app unit tests 8 passed [run](../HistoryTest/runs/2026-11-22_042600Z_app-unit_c727219/RUN.md); UI smoke test 114 of 114 [run](../HistoryTest/runs/2026-11-22_042600Z_ui_c727219/RUN.md).

## 2026-11-22 - Phase I6c-3: the real Node extension host pinned

### Added
- Pins for VSCodium 1.140.06880's server on Linux x64/arm64, macOS x64/arm64 and Windows x64, with their SHA-256 (checked against the published hashes) and the server's commit. "Download and start Node extensions" now works on those systems.
- `core/tests/node_host_real.rs`: an ignored check that downloads, starts and probes the real server.

### Changed
- extensions.md no longer says Node extensions wait for I6c.

### Tests
- Real server on the PC, passed [run](../HistoryTest/runs/2026-11-22_015908Z_node-host_506f165/RUN.md); core unit tests 94 passed [run](../HistoryTest/runs/2026-11-22_015916Z_core_506f165/RUN.md).

## 2026-11-22 - Phase I6c-2: Node extensions in the editor

### Added
- Pheonix picked "VSCodium server, on first need" for Node extensions (design accepted).
- **Node extensions install** from Open VSX (same hash and signature checks) and run only in the local Node extension host of a trusted folder.
- Extensions view > **Node extensions** panel: trust the folder (its own click, with what Node extensions can do), download and start the host (names the size and the hash check), restart, stop, stop trusting. Starting or stopping reloads the window.
- The editor connects to the running host on 127.0.0.1 with its token and pinned commit, and the folder becomes the host's workspace, so Node extensions see real file paths.
- New npm package `@codingame/monaco-vscode-remote-agent-service-override` 39.0.5 (exact), loaded only when a host runs.

### Changed
- CSP `connect-src` adds `ws://127.0.0.1:*` and `http://127.0.0.1:*` for the page; the build removes local sources from the extension host iframe's CSP, so web extensions still can't reach this computer.
- Pins now carry the server's VS Code commit.

### Tests
- Core unit tests 94 passed [run](../HistoryTest/runs/2026-11-22_014615Z_core_10da8d4/RUN.md); UI smoke test 113 of 113 [run](../HistoryTest/runs/2026-11-22_014626Z_ui_10da8d4/RUN.md); app unit tests [run](../HistoryTest/runs/2026-11-22_014647Z_app-unit_10da8d4/RUN.md); static checks [run](../HistoryTest/runs/2026-11-22_014648Z_security-scan_10da8d4/RUN.md).

## 2026-11-21 - Phase I6c-1: Node extension host, core part

### Added
- Design page [features/node-extensions-design.md](features/node-extensions-design.md): Node extensions run in the MIT VSCodium server, downloaded on first need, as a local sidecar for trusted folders only (option A; Pheonix's pick is pending).
- Core module `node_host`: pinned server download with SHA-256 check (no pins yet, so nothing downloads until I6c-3 pins the real files), safe `.tar.gz` unpack, folder trust store, and a sidecar start on `127.0.0.1` with a random connection token in a 0600 file and an allow-listed environment.
- Shell commands `node_host_status`, `node_host_trust`, `node_host_start`, `node_host_stop`. The host stops when another folder opens, when trust is taken back, and when the app exits. The UI doesn't call them yet (I6c-2).
- New dependency: `tar` 0.4.46 (read-only use), with `filetime` 0.2.30.

### Tests
- Core unit tests 93 passed [run](../HistoryTest/runs/2026-11-21_232857Z_core_3e0d986/RUN.md); static checks failed once on stale licence notices [run](../HistoryTest/runs/2026-11-21_232914Z_security-scan_3e0d986/RUN.md), then passed [run](../HistoryTest/runs/2026-11-21_232948Z_security-scan_f363048/RUN.md).

## 2026-11-21 - Phase I6b: internet for web extensions, asked once per extension

### Added
- **Internet for web extensions** ([features/extensions.md](features/extensions.md#internet-for-web-extensions)), Pheonix's choice 2026-11-21: the first time a web extension tries to reach an https site, a card asks "Allow internet for X?" and names the site. Allowed extensions' `fetch` requests are made by the Rust core (public https only). The Extensions view shows which extensions have internet, with **Allow internet** and **Stop internet**.
- Core module `ext_net` (request checks and the request itself); shell commands `ext_set_network` and `ext_fetch`; extension entries gain `network` (off by default and after a reinstall).

### Changed
- The web worker extension host starts from `app/src/extHostWorker.ts`, which loads the request shim before VS Code's worker. The worker's CSP is unchanged (no network of its own).

### Tests
- Core unit tests 87 passed [run](../HistoryTest/runs/2026-11-21_231802Z_core_caf40d4/RUN.md); UI smoke test 105 of 105 [run](../HistoryTest/runs/2026-11-21_231813Z_ui_caf40d4/RUN.md); app unit tests [run](../HistoryTest/runs/2026-11-21_231829Z_app-unit_caf40d4/RUN.md); static checks [run](../HistoryTest/runs/2026-11-21_231829Z_security-scan_caf40d4/RUN.md).

## 2026-11-21 - Phase I6b: web extensions in a Web Worker

### Added
- **Web extensions** ([features/extensions.md](features/extensions.md#web-extensions-i6b)): extensions from Open VSX with a `browser` entry now install and run in VS Code's web worker extension host, a Web Worker in a hidden sandboxed iframe with no network, no Node, no Tauri commands and no page access. They work with the files open in the editor. The Extensions view labels them.
- Removing or turning off a web extension restarts the extension host without it, so its code stops at once.

### Changed
- The core installs extensions with a `browser` entry (also when they have a `main` entry too) as kind `web`; Node-only extensions are still refused (I6c). New refusal: web extensions asking for proposed VS Code APIs.
- CSP ([configuration/tauri-conf.md](configuration/tauri-conf.md)): `'unsafe-eval'` and the extension host iframe's script hash in `script-src`, `frame-src 'self'`, `frame-ancestors 'self'`. The IDE page gets its own stricter meta CSP without `'unsafe-eval'` (`app/vite.config.ts`).

### Tests
- Core unit tests 83 passed [run](../HistoryTest/runs/2026-11-21_215005Z_core_fd65ba7/RUN.md); UI smoke test 98 of 98 with 7 new web extension checks [run](../HistoryTest/runs/2026-11-21_215016Z_ui_fd65ba7/RUN.md); app unit tests [run](../HistoryTest/runs/2026-11-21_215028Z_app-unit_fd65ba7/RUN.md); static checks and scans [run](../HistoryTest/runs/2026-11-21_215028Z_security-scan_fd65ba7/RUN.md).
- Not run yet: the real desktop webviews on the PC and a live web extension install from open-vsx.org.

## 2026-11-21 - Phase I7: TandemForge on this computer as a model engine

### Added
- **TandemForge on this computer** ([features/tandemforge-engine.md](features/tandemforge-engine.md)): pair the IDE with a running TandemForge using the code it shows in Settings > Connected apps; chat and the agent then use TandemForge's local models (Ollama, LM Studio) through the same OpenAI-style API as a gateway. Nothing leaves the computer.
- Core module `tandemforge` (finds TandemForge's `engine.json`, loopback URL only, pairing code check) and gateway kind `tandemforge` (one entry; its port follows TandemForge at each use).
- Shell commands `tandemforge_status`, `tandemforge_connect`; Gateways view panel "TandemForge on this computer".
- Core test `tests/tandemforge_engine.rs` (ignored by default) that runs against a real TandemForge backend.

### Changed
- The HTTP client allows plain http only for loopback URLs the gateway check already allowed (dev gateway, TandemForge); everything else stays https only.

### Tests
- Core unit tests 83 passed [run](../HistoryTest/runs/2026-11-21_205545Z_core_1e284ce/RUN.md); UI smoke test 91 of 91 with 4 new checks [run](../HistoryTest/runs/2026-11-21_205557Z_ui_1e284ce/RUN.md); app unit tests [run](../HistoryTest/runs/2026-11-21_205604Z_app-unit_1e284ce/RUN.md).
- End to end against a real TandemForge backend with a fake Ollama [run](../HistoryTest/runs/2026-11-21_205630Z_tf-engine-e2e_1e284ce/RUN.md); static checks and scans [run](../HistoryTest/runs/2026-11-21_205639Z_security-scan_1e284ce/RUN.md).
- Not run yet: the two desktop apps side by side on the PC with a real model.

## 2026-11-21 - Phase I6a: Extensions from Open VSX, VS Code editor services

### Added
- **Extensions view** ([features/extensions.md](features/extensions.md)): search Open VSX, install with one click, turn extensions on or off, remove them, and pick a colour theme. Phase I6a installs extensions that only contribute files (colour and icon themes, grammars, snippets, language settings); extensions that run code are refused with the phase that adds their host (I6b web worker, I6c Node).
- Core `extensions` module: verified install (published SHA-256 **and** Open VSX Ed25519 signature, manifest must match the request), archive unpacked entry by entry with a path allow-list and size caps, store in the app data folder ([files/app-data.md](files/app-data.md)).
- Shell commands `ext_search`, `ext_install`, `ext_list`, `ext_set_enabled`, `ext_uninstall`, `ext_manifest`, `ext_file`.
- `app/stubs/jschardet`: a stand-in that keeps an unused LGPL-2.1+ dependency out of the installers ([files/jschardet-stub.md](files/jschardet-stub.md)).

### Changed
- The editor runs on `@codingame/monaco-vscode-api` 39.0.5 (VS Code's editor services): VS Code's TextMate grammars for about 30 languages and its Dark Modern theme. File language ids are VS Code's (`.tsx` is `typescriptreact`, shell scripts `shellscript`).
- CSP: `'wasm-unsafe-eval'` in `script-src`; `'self' data: blob:` in `connect-src`; `blob:` in `img-src` and `font-src` ([configuration/tauri-conf.md](configuration/tauri-conf.md)).
- `dompurify` is now an npm override (3.4.18) instead of the patched copy in `vite.config.ts`.
- New crates `zip` 9.8.1 (deflate only) and `flate2` 1.1.11 (pure Rust); THIRD-PARTY-NOTICES.txt regenerated (527 packages).

### Tests
- Core unit tests with 9 new extension tests [run](../HistoryTest/runs/2026-11-21_173347Z_core_848b60e/RUN.md); real install path on Open VSX files cached with curl (Dracula installed with a verified signature, two Node extensions refused) [run](../HistoryTest/runs/2026-11-21_173357Z_ovsx-cached_848b60e/RUN.md).
- UI smoke test 87 of 87 with 6 new Extensions checks, under the shipped CSP [run](../HistoryTest/runs/2026-11-21_173412Z_ui_848b60e/RUN.md); app unit tests [run](../HistoryTest/runs/2026-11-21_173418Z_app-unit_848b60e/RUN.md).
- tsc, rustfmt, clippy, `cargo audit` (no vulnerabilities), npm audit gate, licence gate [run](../HistoryTest/runs/2026-11-21_173522Z_security-scan_848b60e/RUN.md).
- Not run yet: a live install from open-vsx.org on the PC (the cloud container reaches the internet only through a proxy, which the IDE does not use).

## 2026-11-21 - Roadmap checklist in this repository

### Added
- [ROADMAP.md](ROADMAP.md): every phase of the IDE plan (I0 to I7, side track X, accounts) with its status and the pull request that shipped it. I0 to I5 are done; I6 extensions and I7 are next. Documentation only.

### Changed
- README.md and features/missions.md point to the checklist instead of an out-of-date "next" line.

## 2026-11-17 - README and user guide with screenshots

### Added
- [user-guide.md](user-guide.md): a screenshot tour of every view (editor, search, source control, gateways, agent chat, change review, missions, run and test, run approval), written for people new to the IDE.
- `docs/images/` (10 pictures) and `app/tests/docs-shots.mjs`, which retakes them from the built webview ([commands/docs-screenshots.md](commands/docs-screenshots.md)).

### Changed
- README.md rewritten for new users: what the IDE does, a feature table with screenshots, getting started, and an up-to-date status (it still said phase I1).

### Tests
- Screenshots: [run](../HistoryTest/runs/2026-11-17_225231Z_docs-shots_64acea3/RUN.md), 10 pictures, exit 0. UI smoke test still passes: [run](../HistoryTest/runs/2026-11-17_225240Z_ui_64acea3/RUN.md), 81 of 81.

## 2026-11-17 - npm audit gate with dated dev-only exceptions; picomatch ReDoS logged as an accepted risk

### Changed
- CI's supply-chain job and the local CI run `scripts/npm_audit_gate.py app` instead of `npm audit --audit-level=high`: shipped dependencies stay strict, and dev dependencies too, except advisories listed with an expiry in `app/npm-audit-exceptions.json` ([commands/npm-audit-gate.md](commands/npm-audit-gate.md)). The owner chose this option. Tests: [run](../HistoryTest/runs/2026-11-17_235209Z_security-scan_dc6d647/RUN.md).

### Security
- GHSA-fc5q-39xc-vjpf (picomatch ReDoS, CVE-2026-112366) has no fixed release; it reaches only build tooling (`vite`, `@tailwindcss/vite`), not the installers. Logged as an accepted risk in [SECURITY-LOG.md](SECURITY-LOG.md), to revisit when picomatch ships a fix. Run: [HistoryTest/runs/2026-11-17_232934Z_security-scan_64acea3](../HistoryTest/runs/2026-11-17_232934Z_security-scan_64acea3/RUN.md).

## 2026-11-17 - Third-party notices brought up to date

### Fixed
- `app/src-tauri/THIRD-PARTY-NOTICES.txt` regenerated: the I5c and I5d sandbox crates (12 more, 479 packages in all) were missing, so the release build's licence gate refused to build the installers on the PC. Every licence is on the allow-list. Run: [HistoryTest/runs/2026-11-17_193821Z_installers_377af6f](../HistoryTest/runs/2026-11-17_193821Z_installers_377af6f/RUN.md).

## 2026-11-17 - Phase I5d: Local container runner (Docker or Podman)

Requested by Pheonix on 2026-11-17: "include support for local container like docker or podman". Images follow the
recommended option (Red Hat UBI per framework; decision pending).

### Added
- **Local container, Docker or Podman** runner in Run and test and for the chat's runs: each run in a throw-away
  container on a copy of the folder, with no network, no capabilities, not as root, a read-only image and limits;
  Podman preferred, then Docker ([features/container-runner.md](features/container-runner.md)).
- Four images pinned by digest: UBI 9 Python 3.12, Node.js 22, Go toolset, and a Rust image built here from the
  pinned UBI 9 base. Tests pick the image from the framework; commands use the **Image** list.
- **Get image** in Run and test: the only way an image is downloaded or built; runs use `--pull=never`.
- Core module `container` (engine, images, prepare, run arguments, run); shell commands `container_images` and
  `container_prepare`; `run_start` and `agent_send` take the image.
- `core/examples/local_containers.rs` ([commands/local-containers.md](commands/local-containers.md)).
- Tests: core unit tests 68 -> 70, UI smoke 77 -> 81, real containers (Docker, Linux and Windows).

### Changed
- The snapshot, cache and output helpers of `localbox` are shared with the container runner; the container copy
  includes `node_modules` and `.venv` instead of linking them.

## 2026-11-17 - Phase I5c: Local sandbox on Windows (AppContainer)

Requested by Pheonix on 2026-11-17 ("include local os sandbox ported from tandemforge"). A Rust port of
TandemForge's `hostrun/winsandbox.py`, in toolchain mode.

### Added
- **Local sandbox on this computer** on Windows 10 (1703+) and 11: each run in an AppContainer with no capabilities
  (no network, loopback included) and a job object (kill on close, process, memory, CPU and UI limits); folders
  shared with the sandbox's own SID only for the run (recorded in `run-grants.json`, removed afterwards); `.exe`
  programs only, `npm` through `node.exe`; the container's own folder emptied after each run
  ([features/local-sandbox.md](features/local-sandbox.md)).
- Core module `localbox_win` (profile, grants, job object, process start, command-line quoting, environment block).
- Tests: `local_runs_are_confined_on_windows` (real Windows), quoting and environment-block tests; the
  `local_toolchains` example finds Windows programs as the sandbox does.

### Changed
- `windows-sys` 0.61.2 is a direct dependency of the core on Windows (already in the lockfiles).
- On Windows the snapshot copies `node_modules` and `.venv` instead of linking them.

## 2026-11-17 - Phase I5c: Local sandbox on this computer (Linux)

Requested by Pheonix on 2026-11-17: "include local os sandbox ported from tandemforge". A Rust port of TandemForge's
host runner (`hostrun/confine.py`) in toolchain mode (the recommended option; decision pending).

### Added
- **Local sandbox on this computer** runner in Run and test and for the chat's runs: a copy of the folder, run in
  new user, network, PID, mount, IPC, UTS and cgroup namespaces with Landlock, seccomp, rlimits and a wall-clock
  limit; no network, home folder hidden except toolchain folders (read only), changes thrown away
  ([features/local-sandbox.md](features/local-sandbox.md)).
- Core modules `localbox` (`available`, `run`) and `testparse` (framework commands and test counts, as the
  gateway's); `store::random_id`.
- Shell command `runners_list`; `run_start` and `agent_send` take the runner (`gateway` or `local-os`); the approval
  card says where the run happens.
- Per-folder build cache under `run-cache/` ([files/app-data.md](files/app-data.md)).
- `core/examples/local_toolchains.rs` ([commands/local-toolchains.md](commands/local-toolchains.md)).
- Tests: core unit tests 61 -> 68 (incl. the real-kernel `local_runs_are_confined`), UI smoke 75 -> 77, real toolchains 6/6.

### Changed
- `libc` 0.2.191 is a direct dependency of the core on Unix (already in the lockfiles).

## 2026-11-17 - Phase I5b: Run and test on the gateway sandbox

Requested by Pheonix (I5 order chosen 2026-11-17: gateway sandbox first, then a local OS sandbox ported from
TandemForge, then Docker/Podman).

### Added
- **Run and test** view: run the project's tests (cargo, go, make, npm, pytest, unittest) or one program (argument
  list, no shell) in the linked project's gateway sandbox; verdict, test counts, failing tests and output as text.
  Local sandbox and container are listed as coming next.
- Agent chat **Runs** switch (off by default): the assistant may ask for `run_tests` or `run_command`; each request
  shows an approval card and runs only after **Allow run**.
- Core module `runner` (`check`, `split_words`, `gateway_run`, `RunResult`); agent run tools behind a runner seam
  (`RunFn`), `tool_defs_for`, `system_prompt`; `Client::api_slow` (long calls).
- Shell commands `run_start` and `agent_decide`; `agent_send` takes `runs`.
- `core/examples/live_runs.rs` and `scripts/live_gateway_e2e.py --example`.
- Tests: core 64 -> 68 (incl. run tools through the runner only), UI smoke 64 -> 75, live gateway runs 13/13.
- Docs: `features/run-and-test.md`; `commands/live-gateway-e2e.md` updated.

### Changed
- The shared push flag (`sync_busy`) also covers runs, so a send, pull or run happen one at a time.

## 2026-11-17 - Installers and signed releases

Requested by Pheonix (thread "Ship signed installers and releases"). No Authenticode yet (Pheonix's choice); cosign-signed
`SHA256SUMS`.

### Added
- Installer settings in `tauri.conf.json` (publisher, licence page, per-user NSIS install, SHA-256 signing digest) and
  the licence texts as app resources: `LICENSE`, `NOTICE` and `app/src-tauri/THIRD-PARTY-NOTICES.txt`
  ([files/third-party-notices.md](files/third-party-notices.md)).
- Release procedure through the P2P-AI repository's `scripts/release_app.py --app ide` ([features/installers.md](features/installers.md)).

### Tests
- Cloud: licence gate, 467 packages all allowed ([run](../HistoryTest/runs/2026-11-17_085507Z_installers_1b95d91/RUN.md)); Linux .deb/.rpm
  build with the licence files installed ([run](../HistoryTest/runs/2026-11-17_085509Z_installers_1b95d91/RUN.md)). Windows installers:
  pending the PC run.

## 2026-11-17 - Workspace file access through a folder handle

Part of the security to-do sweep Pheonix asked for (2026-11-17).

### Security
- Every workspace read, listing, search, byte read for project sync and save now opens through a cap-std handle on the
  opened folder instead of checking a canonical path and then opening it. A folder swapped for a link between the two
  can no longer redirect the IDE outside the folder. Closes the `TODO(security)` in `core/src/workspace.rs`.

### Added
- Dependency `cap-std` 4.0.3 (core).

### Tests
- Core: 66 tests (2 new: folder swapped for a link after open is refused; workspace follows its renamed folder), fmt,
  clippy ([run](../HistoryTest/runs/2026-11-17_125223Z_core_6ebd731/RUN.md)). OSV: no advisories for the new crates ([run](../HistoryTest/runs/2026-11-17_125233Z_security-scan_6ebd731/RUN.md)).
  The Tauri shell could not build in the cloud container (no GTK libraries; marked invalid, [run](../HistoryTest/runs/2026-11-17_125226Z_shell_6ebd731/RUN.md)); CI builds it.

## 2026-11-17 - Phase I5a: Source Control view

Requested by Pheonix ("keep moving forward with the rest"; I5 order chosen 2026-11-17: gateway sandbox first, then a
local OS sandbox ported from TandemForge and Docker/Podman). Source control is the part of I5 that needs no runner.

### Added
- **Source control** view: branch with ahead/behind, Staged and Changes lists, view-only diff against the last
  commit, stage/unstage one file or all, commit with a message, the last 50 commits. Local only (no fetch, pull, push).
- Core module `scm` (the installed git, with repository programs and hooks off); `Workspace::check_rel`; shell
  commands `scm_status`, `scm_diff`, `scm_stage`, `scm_unstage`, `scm_commit`, `scm_log`.
- Tests: core 60 -> 64 (incl. a real-git round trip), UI smoke 57 -> 64.
- Docs: `features/source-control.md`.

### Fixed
- The real-git test now uses a file named `[a].txt` (a valid name on Windows that git would read as a pattern)
  instead of `*`, which Windows cannot create; the first Windows run failed there, not in product code.

### Changed
- `DiffReview` can be view-only (no Accept or Reject) with its own labels.

## 2026-11-17 - Phase I4: missions (Autopilot and Auto Improve)

Requested by Pheonix ("keep moving forward with the rest").

### Added
- **Missions** view: link the open folder to a gateway project (pick or create), **Send folder to the project**
  (changed files only, secrets kept local, report of skipped files), **Autopilot** and **Auto Improve** switches,
  start a mission with a goal, model and 1-8 acceptance checks, Pause, Resume, Stop (two clicks), approval review
  with the exact arguments, proposals (Accept, Dismiss, Snooze a week), and **Bring the work back** (Collect changes,
  Review changes into the existing diff review).
- Status bar **Autopilot** and **Auto Improve** chips show the project's switches and open Missions.
- Core modules `gwapi` (gateway API calls, multipart upload, capped download, error codes to fixed messages) and
  `projects` (push, pull, folder links, view shapes, acceptance checks); `agent::propose_external`;
  `Workspace::all_files` and `read_bytes`. 16 shell commands in `missions.rs`.
- `links.json` in the app data folder (folder-to-project links).
- Tests: core 49 -> 60 (incl. a fake-gateway integration test), UI smoke 45 -> 57.
- Docs: `features/missions.md`, `commands/live-gateway-e2e.md`.
- Live check `scripts/live_gateway_e2e.py` with `core/examples/live_missions.rs`: a real gateway, node and sandbox
  run an Autopilot mission end to end from the IDE core (19 of 19).
- Windows build and tests pass at bfae51f (`HistoryTest/runs/2026-11-17_085943Z_windows_bfae51f`).

### Changed
- `ChatView` refreshes **Changes to review** when work is brought back from a mission.

## 2026-11-17 - Phase I3: chat tools and diff review

Requested by Pheonix ("keep moving forward").

### Added
- Agent chat **Tools** switch (on by default with a folder open): the model can list, read and search the folder and
  propose edits and new files through the gateway's tools API; each tool step shows in the chat.
- **Changes to review** list and a side-by-side Monaco diff over the editor; **Accept and save** (version-checked) or
  **Reject** per file; an open tab reloads after an accept.
- Core module `agent` (tools, deny list for likely secrets, pending changes, turn loop with budgets); streamed
  `tool_calls` in the client; shell commands `agent_send`, `changes_list`, `change_get`, `change_accept`,
  `change_reject`. Core tests 41 -> 48, UI smoke 37 -> 45.
- Docs: `features/agent-tools.md`.

### Changed
- Saved chats may hold `tool` lines (never sent to a model).

### Fixed
- Switching from Agent chat to another view and back lost the chat in progress (the next answer started a new saved
  chat); the chat view now stays mounted.
- Windows: `edit_file` never matched a multi-line change in a file with CRLF line endings, because models send LF.
  The edit now adapts to the file's line endings and keeps them (found in the Windows click-through, core tests
  48 -> 49).

## 2026-11-17 - App icons: the P2P-AI logo with the IDE badge

Requested by Pheonix: one logo (the brain with network nodes) for every P2P-AI app, with something different per app.

### Changed
- `app/src-tauri/icons`: the logo with a purple `</>` badge replaces the placeholder; `bundle.icon` now lists the
  full Tauri set (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.icns`, `icon.ico`). Built by
  `branding/make_icons.py --ide-root` in the P2P-AI repository ([files/app-icons.md](files/app-icons.md)).

### Test evidence
- `HistoryTest/runs/2026-11-17_062058Z_branding_ea76006` (icons up to date and verified, imported from the P2P-AI run).
## 2026-11-17 - Phase I2b: public gateway list

Requested by Pheonix ("keep moving forward").

### Added
- Gateways view, **Public gateways**: a directory list (none by default), **Refresh list**, entries with region,
  latency, models, access mode and the prompt-logging statement, **Add** (saved as kind `public`).
- Core module `discovery`: the P2P-AI node's advert verifier ported (strict and canonical JSON, ed25519, id bound to
  key, endpoint and `client_access` rules), signed latency probe, `directories.json`, a public-only resolver.
- Shell commands `directories_list`, `directories_set`, `public_list`, `gateway_add_public`.
- Tests: the shared protocol vectors (copied to `core/tests/vectors/`) and a fake directory; core tests 31 -> 41, UI
  smoke test 30 -> 37 checks.
- Docs: `features/public-gateways.md`, `files/discovery-vectors.md`; `files/app-data.md` gains `directories.json`
  and kind `public`.

### Changed
- Gateways of kind `public` are only ever reached at public addresses (also for chat and Check).
- **Unpin** is offered for any pinned gateway that is not the local one.

### Security
- New dependencies ed25519-dalek 2.2.0, base64 0.22.1, getrandom 0.2.17 (already in the tree); cargo audit: no
  advisories for them.

## 2026-11-17 - Phase I2a: gateways and chat

Requested by Pheonix ("keep moving forward" after phase I1).

### Added
- Gateways view: add the gateway on this computer (from its setup folder: port and CA) or one by address; key in the
  OS keychain; active gateway; Check (latency, models); pin a CA; replace key; remove with two clicks.
- Agent chat: model picker, streamed answers over a Tauri channel, Stop, answers shown as text with code blocks.
- Chat history per open folder on this computer: list, reopen, search, delete with two clicks.
- Status bar gateway chip (opens the Gateways view; INSECURE — DEV ONLY badge for a plain-http loopback gateway).
- Core modules `gateways`, `client`, `chats`, `store`; 21 new core tests (fake gateway over loopback); UI smoke test
  grew from 17 to 30 checks.
- Docs: `features/gateways-and-chat.md`, `files/app-data.md`.

### Security
- New dependencies ureq 3.5.2 (rustls), serde_json, keyring 4.2.0; accepted RustSec "unmaintained" notices for
  zbus 5's `zvariant`, `zvariant_derive`, `zbus_names` on Linux (SECURITY-LOG).

## 2026-11-17 - Phase I1: editor shell

Requested by Pheonix (IDE plan, ADR-029 in the P2P-AI repository; mockup approved 2026-11-17).

### Added
- Rust core `p2p-ide-core`: workspace folder with confined paths, folder listing, text reads, atomic saves with a
  SHA-256 version check, plain-text search, with caps on every size.
- Tauri shell with six typed commands and the native folder picker (called from Rust only).
- React webview in a VS Code-style layout: activity bar, explorer, search, Monaco editor tabs with unsaved markers,
  Ctrl+S, conflict banner, status bar; Chat, Missions, Gateways and Extensions shown as coming in later phases.
- CI: core and shell lint and tests, webview build and UI smoke test, npm audit and cargo audit.
- Windows check: built, tested and clicked through on a Windows 11 PC (HistoryTest runs
  `2026-11-17_051458Z_windows_ba63e5c`, `_054000Z_windows_ba63e5c`, `_054500Z_windows_e30cb3e`).
- `HistoryTest/` and the recorder `scripts/history_test.py` (copied from the P2P-AI repository), `CLAUDE.md`, docs.

### Fixed
- Blank window in release builds: `freezePrototype` is off because Monaco cannot load with a frozen
  `Object.prototype`; the UI smoke test now applies the freeze when the config asks for it.
- Status bar showed the previous tab's language after switching tabs until the cursor moved.

### Security
- Monaco's vendored DOMPurify 3.4.15 is replaced by the patched 3.4.18 at build time (SECURITY-LOG).
- `style-src 'unsafe-inline'` in the CSP for Monaco (accepted risk, SECURITY-LOG).

### Test evidence
- `HistoryTest/runs/2026-11-17_045740Z_core_fe528ba`, `..._045741Z_shell_fe528ba`, `..._045746Z_webview_fe528ba`,
  `..._045806Z_ui-smoke_fe528ba` (16/16), `..._045808Z_security-scan_fe528ba` (npm audit 0),
  `..._045933Z_security-scan_fe528ba` (cargo audit: 0 vulnerabilities, 2 upstream warnings).
