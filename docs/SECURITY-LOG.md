# Security log

Every security patch, hardening change, CVE remediation and accepted risk, newest first.

## 2026-11-23 — Tool calls written as reply text are read under the same checks
- **Type:** hardening (behaviour change with its controls)
- **CVE / CWE / advisory:** none; controls for CWE-20, CWE-77, CWE-400, OWASP LLM01
- **Severity:** n/a (no vulnerability; a model's text could already only become a pending proposal)
- **Affected:** `core/src/agent.rs` (`text_tool_calls`, `run_turn`); also `app/src-tauri/src/missions.rs` (`no_projects` probe, fixed path, error kind only, CWE-209)
- **What changed:** when a step has no real tool calls, a reply that is entirely a JSON tool call (optionally in `<tool_call>` tags or one code fence) is treated as the call.
- **Why it is secure now:** the reply must be nothing but the call (no prose around it), at most 64 KiB and 8 calls, and each name must be a tool offered this turn (run tools only when Runs is on); arguments must be an object. The call then takes the normal path: strict argument checks, read tools on the folder allow-list, edits as pending changes the person accepts, runs only after per-run approval. So a file that makes the model echo JSON gains nothing it could not already get with a real tool call (OWASP LLM01, NIST AC-6).
- **Verification:** core unit tests `calls_written_as_text_are_read_only_when_they_are_the_whole_reply` and `a_turn_takes_a_call_written_as_text`; PC run to be archived in HistoryTest.

## 2026-11-23 — HistoryTest binary artifacts excluded from line-ending conversion
- **Type:** hardening
- **CVE / CWE / advisory:** none; CWE-354 (improper validation of integrity check value) as the failure mode prevented
- **Severity:** n/a (record integrity, no product code)
- **Affected:** `.gitattributes`, `HistoryTest/INDEX.csv`
- **What changed:** `HistoryTest/**/*.{png,jpg,jpeg,gif,webp,pdf,zip,gz}` are `binary`, so git never converts line endings inside them. The index was regenerated from the run folders.
- **Why it is secure now:** archived evidence keeps the exact bytes its manifest hashes, so the SHA-256 chain stays verifiable (NIST AU-9, AU-10).
- **Verification:** `git ls-files --eol HistoryTest`: all 34 images `-text`; the PNG's SHA-256 matches its manifest; `history_test.py verify`: 171 runs, the only problem is the chain note on `2026-11-23_035621Z`, which was already marked invalid.

## 2026-11-23 — NEXUS Engine support: keyless loopback boundary and studio settings discovery
- **Type:** hardening (new local engine integration)
- **CVE / CWE / advisory:** none; controls for CWE-20, CWE-59, CWE-79, CWE-209, CWE-287, CWE-400, CWE-522, CWE-601, CWE-918, NIST SP 800-53 SC-7 / IA-5
- **Severity:** n/a (new feature)
- **Affected:** `core/src/nexus.rs` (new), `core/src/gateways.rs`, `core/src/client.rs`, `core/src/gwapi.rs`, `app/src-tauri/src/{net,main}.rs`, `app/src/{ipc,components/{NexusConnect,NetworkView,ChatView,MissionsView}}.tsx`, `vscode-extension/src/{nexus,gateways,extension,chatView,missionsView}.ts`, `vscode-extension/src/gateway/{checks,client}.ts`
- **What changed:** added discovery and health verification for local NEXUS Proxy (`127.0.0.1:11435`) and NEXUS Runtime (`127.0.0.1:11436`) without API keys.
- **Why it is secure now:**
  - `Kind::Nexus` / `kind: "nexus"` enforces canonical `http://127.0.0.1:<port>` (`1024..=65535`, no path/query/userinfo) on save, on `GatewayStore::open` / `GatewayStore.list()`, and in `Client::new` / `GatewayClient` before any HTTP request (`NIST SC-7`, `CWE-918`).
  - `key = None` is permitted **only** when `kind == Kind::Nexus` on a verified `127.0.0.1` loopback URL (`Authorization` header omitted); all remote, manual, `Local`, and `TandemForge` gateways continue to fail closed (`NetError::NoKey`) without credentials (`CWE-287`).
  - Reading `~/.nexus-studio/studio-settings.json` is capped at 64 KiB and refuses symlinks (`store::read_capped` / `fs.lstatSync`, `CWE-59`, `CWE-400`); HTTP probes use 2 s timeouts, 64 KiB body caps, `max_redirects(0)`, and no env proxy (`CWE-601`); account links (`gateway_set_account`) are refused on `Kind::Nexus` so remote bearer tokens can never be sent to local engines (`CWE-522`).
  - Telemetry strings (`simd`, model modes) are ASCII-sanitized and rendered strictly as React / DOM text (`CWE-79`); errors use fixed strings with no raw paths or URLs (`CWE-209`).
- **Verification:** HistoryTest runs `2026-11-23_114345Z_core_dc07c54` (core), `2026-11-23_114348Z_live-nexus_dc07c54` (live NEXUS proxy + runtime E2E), `2026-11-23_114353Z_shell_dc07c54` (Tauri shell), `2026-11-23_114250Z_ui_dc07c54` (UI smoke 133/133), `2026-11-23_114356Z_vscode-ext-unit_dc07c54` (extension 48/48), `2026-11-23_114426Z_security-scan_dc07c54`, and `2026-11-23_114428Z_vscode-ext-audit_dc07c54`.

## 2026-11-23 — Agent commits: applying a gateway patch to the person's repository
- **Type:** hardening (new feature with its controls)
- **CVE / CWE / advisory:** none; controls for CWE-20, CWE-22, CWE-59, CWE-73, CWE-77, CWE-88, CWE-367, CWE-460, CWE-829
- **Severity:** n/a (new code)
- **Affected:** `core/src/scm.rs` (`check_patch`, `apply_checkpoint`, `run_with`), `core/src/projects.rs` (`checkpoint_view`), `app/src-tauri/src/agent_commits.rs`, `app/src/components/AgentCommits.tsx`
- **What changed:** a patch from the gateway is applied only after the person's click, only after `check_patch` passes (plain edits of allow-listed paths; no renames, modes, links, binaries; exact hunk counts; no `.git`, `..`, absolute, `\` or `:` paths), and with nothing of the person's staged. Patch and message reach git on standard input. Hooks and repository programs stay off. A failed commit is undone. The outcome is reported once.
- **Why it is secure now:** the gateway cannot make the IDE write outside the folder or run anything (CWE-22, CWE-88; OWASP LLM06); the person decides each commit (NIST AC-3).
- **Verification:** core tests `checkpoint_patches_are_checked_and_cut_to_the_diff` (15 hostile patches) and `real_git_applies_an_agent_checkpoint` (real git: staged refusal, not-fitting patch changes nothing, hooks do not run, message kept); runs in HistoryTest.
## 2026-11-23 — VS Code extension: sign in with a P2P-AI account (A3)
- **Type:** hardening (new component, authentication)
- **CVE / CWE / advisory:** none. CWE-20, CWE-208, CWE-338, CWE-346, CWE-352, CWE-400, CWE-522, CWE-532, CWE-601, CWE-918
- **Severity:** none (new feature, used only when a gateway is linked to an account). KEV: no.
- **Affected:** `vscode-extension/src/accounts.ts` (new), `vscode-extension/src/{gateways,extension,chatView,missionsView}.ts`, `vscode-extension/src/gateway/client.ts`, `.gitattributes` (new)
- **What changed:** the extension gets the same controls as the IDE: sign-in only in the system browser (`openExternal`), PKCE S256 and `state` from `crypto.randomBytes`, `state` compared with `timingSafeEqual`, `iss` checked, repeated parameters refused, a 127.0.0.1 listener on a random port for one callback with a 5-minute limit and Cancel. Home calls use a fresh agent with no proxy from the environment, no redirects, a 64 KiB answer cap and timeouts. Refresh tokens are kept only in VS Code SecretStorage and stored again before use after each rotation, under one lock; account tokens stay in memory, must be visible ASCII of at most 4096 characters before they go into a header, and never reach webviews, settings or logs. Output channel audit lines use fixed words. `.gitattributes` keeps HistoryTest files LF so their SHA-256 chain holds on Windows.
- **Why it is secure now:** RFC 8252 and RFC 7636 (OWASP ASVS V51), protected authenticators (NIST IA-5, CWE-522), header injection blocked (CWE-20), SSRF limits on home calls (CWE-918), audit of authentication events (NIST AU-2), record integrity (NIST AU-10).
- **Verification:** on the Windows PC: `npm ci` (0 vulnerabilities), HistoryTest `2026-11-23_035759Z_vscode-ext-check_9243fa4` (tsc 0 errors) and `2026-11-23_035800Z_vscode-ext-unit_9243fa4` (44 of 44).

## 2026-11-23 — Accounts (ADR-030 A3): native sign-in and account tokens for gateways
- **Type:** hardening (new component, authentication); accepted risk
- **CVE / CWE / advisory:** none. CWE-20, CWE-78, CWE-79, CWE-200, CWE-208, CWE-294, CWE-346, CWE-352, CWE-400, CWE-522, CWE-532, CWE-601, CWE-613, CWE-918
- **Severity:** none (new feature, used only when a gateway is linked to an account). KEV: no.
- **Affected:** `core/src/accounts.rs` (new), `core/src/gateways.rs` (`account` link), `app/src-tauri/src/{account,net,main}.rs`, `app/src/components/{AccountsPanel,NetworkView}.tsx`, `app/src/ipc.ts`
- **What changed:** sign-in runs in the system browser, never an embedded view, so the app never sees the password (RFC 8252 8.12). PKCE S256 with a 32-byte CSPRNG verifier; a 32-byte `state` compared in constant time; the `iss` answer checked against the home (RFC 9207 mix-up defence); repeated or missing parameters refused; the loopback listener is bound to 127.0.0.1 on a random port, takes one callback, caps connections and gives up after 5 minutes or on Cancel. Account service addresses must be https (plain http only for 127.0.0.1, shown as INSECURE — DEV ONLY), have no user, query or fragment, and only allow-listed path characters. Refresh tokens are kept only in the OS keychain under their own service and are stored again before use after each rotation; a reused or revoked token clears the session. Access tokens stay in memory, are scoped to one gateway id (`resource`), and are never sent to the webview, the Node extension host, logs or errors. Sign-out revokes at the home. An operator-made one-time password must be replaced before any gateway token. The browser is started without a shell, and only for the checked home's `/authorize?` link. Audit lines use fixed words only.
- **Why it is secure now:** OAuth for native apps per RFC 8252 and RFC 7636 (OWASP ASVS V51), protected authenticators (NIST IA-5, CWE-522), short-lived scoped tokens (NIST IA-2, AC-6), session ending on revocation (CWE-613), injection-free browser launch (CWE-78), audit of authentication events (NIST AU-2, AU-3).
- **Accepted risk:** a gateway does not yet publish which account service it trusts or its account id, so the person types both; a wrong id only gets "not accepted" from the gateway. Refresh tokens live as long as the account service's session policy allows.
- **Verification:** HistoryTest `2026-11-23_031826Z_core_ffb62c3` (134 passed), `2026-11-23_031839Z_ui_ffb62c3` (126 of 126), `2026-11-23_031903Z_live-accounts_ffb62c3` (19 of 19, including a token for another gateway id refused), `2026-11-23_032814Z_security-scan_ffb62c3` (cargo audit: 0 vulnerabilities, 6 pre-existing allowed warnings in the Tauri Linux stack; npm audit: 0).

## 2026-11-22 — Phase I6d: Electron compatibility host
- **Type:** hardening (new component, supply chain); accepted risk
- **CVE / CWE / advisory:** none. CWE-22, CWE-59, CWE-79, CWE-94, CWE-250, CWE-276, CWE-362, CWE-409, CWE-494, CWE-522, CWE-601, CWE-732, CWE-918
- **Severity:** none (new feature, off by default). KEV: no.
- **Affected:** `core/src/electron_host.rs` (new), `core/src/node_host.rs` (`Launcher.env`, `safe_parts` shared), `core/examples/node_host_serve.rs`, `app/src-tauri/src/{nodehost,main}.rs`, `app/src/components/NodeHostPanel.tsx`, `app/src/ipc.ts`
- **What changed:** an opt-in switch runs the Node extension host's pinned server with Electron 43.8.3 in Node mode. Electron comes from Electron's own GitHub release, pinned per OS and CPU by SHA-256 (taken from our download, equal to the release's `SHASUMS256.txt`), and is downloaded only after the person agrees. The unzip allow-lists paths, refuses links, caps entries and bytes, and never unpacks the set-uid `chrome-sandbox`. Files are owner-only. The only addition to the scrubbed environment is the fixed `ELECTRON_RUN_AS_NODE=1`. Switch changes are audited. The native-module scan only reads, with caps and without following links.
- **Why it is secure now:** integrity of the new binary (NIST SI-7, SA-12, SSDF PS.2), least function by default (NIST CM-7), safe archive handling (CWE-22, CWE-59, CWE-409), no privilege-raising helper (CWE-250).
- **Accepted risk:** Electron 43.8.3 is the version VS Code 1.145.2 ships. A live advisory lookup was not done from the cloud; re-check it on every editor library upgrade. Only its Node runtime is used. Native modules run with the person's rights, as Node extensions already do, and only in a trusted folder.
- **Verification:** HistoryTest `2026-11-22_121508Z_electron-pin_01bc30e`, `2026-11-22_121508Z_core_01bc30e` (116 passed), `2026-11-22_121522Z_ui_01bc30e` (118 of 118), `2026-11-22_121549Z_node-host-electron_01bc30e` and `2026-11-22_121624Z_node-host-electron_01bc30e` (real server: activates under Electron, expected failure on plain Node), `2026-11-22_121745Z_security-scan_882c782` (cargo audit, npm audit gate, licence gate: pass).

## 2026-11-22 — Side track X3: missions in the VS Code extension; settings file
- **Type:** hardening (new component); patch (settings race)
- **CVE / CWE / advisory:** none. CWE-20, CWE-22, CWE-59, CWE-74, CWE-79, CWE-150, CWE-178, CWE-200, CWE-276, CWE-312, CWE-345, CWE-362, CWE-367, CWE-400, CWE-918; OWASP LLM06
- **Severity:** settings race: low (a setting could come back or vanish, for example a removed gateway reappearing; no key was ever involved, keys are in SecretStorage). KEV: no.
- **Affected:** `vscode-extension/src/missions/projects.ts`, `src/missionsView.ts`, `src/fileState.ts` (new), `src/gateways.ts`, `src/chatView.ts`, `src/extension.ts`, `src/viewMessages.ts`, `src/gateway/{checks,client}.ts`, `src/agent/{tools,workspace}.ts`, `media/missions.js`, `package.json`; `core/src/projects_vectors.rs` (tests only)
- **What changed:** missions run only on a trusted local folder. Push applies the agent deny list and the gateway's path grammar, re-hashes each file before upload and commits with `base_version`. Pull checks each download against the listed SHA-256, takes only UTF-8 text and only makes pending changes the person accepts per file. Ids are checked before they go into a URL. Every gateway field is cleaned and drawn as text. An approval carries the digest of the arguments the extension fetched, not one from the webview. Audit lines use fixed words only. Settings moved from `globalState` to `state.json` (atomic `wx` temp file and rename, mode 0600, folder 0700, read capped at 1 MiB, values re-checked on read).
- **Why it is secure now:** human approval bound to what was shown (OWASP LLM06, CWE-345), integrity of transfers (NIST SI-7), least data to the gateway (CWE-200), input validation at the boundary (NIST SI-10), and settings that can't be undone by another process's stale copy (CWE-362).
- **Verification:** HistoryTest `2026-11-22_114442Z_core_ae1f4aa`, `2026-11-22_115617Z_vscode-ext-unit_0c3ceb5` (35 of 35), `2026-11-22_115617Z_vscode-ext-integration_0c3ceb5` (1.147.0, stable over 10 repeats; 20 earlier repeats with 0 failures), `2026-11-22_115715Z_vscode-ext-integration_0c3ceb5` (1.95.0), `2026-11-22_114505Z_vscode-ext-ui_ae1f4aa`, `2026-11-22_115718Z_vscode-ext-audit_0c3ceb5` (npm audit: 0). The failing run `2026-11-22_114456Z_vscode-ext-integration_ae1f4aa` is kept with a note.

## 2026-11-22 — Side track X2: agent tools in the VS Code extension
- **Type:** hardening (new component); accepted risk
- **CVE / CWE / advisory:** none. CWE-20, CWE-22, CWE-59, CWE-73, CWE-79, CWE-200, CWE-367, CWE-400, CWE-538, CWE-770; OWASP LLM01, LLM06
- **Severity:** none (new feature). KEV: no.
- **Affected:** `vscode-extension/src/agent/{workspace,tools}.ts`, `src/review.ts`, `src/chatView.ts`, `src/extension.ts`, `src/viewMessages.ts`, `src/gateway/{checks,client}.ts`, `media/chat.js`, `package.json`; `core/src/agent_vectors.rs` (tests only)
- **What changed:** the extension's chat can give the model the IDE's five folder tools. They are read-only on disk: edits and new files only become pending changes. A change is written only when the person accepts it, through an atomic write with a SHA-256 version check, so a file changed meanwhile is refused and a new file never replaces one that appeared. The tools run only in a trusted workspace on a local folder. The same deny list as the IDE keeps likely secrets out of reads, listings and search. Paths pass the IDE's allow-list, and links are never followed. Tool arguments are strict JSON, and every budget from the IDE applies. The diff is VS Code's read-only text diff of in-memory documents, and the view can only name a pending change.
- **Why it is secure now:** human approval for every write (OWASP LLM06, NIST AC-6), least data to the gateway (CWE-200), confinement (CWE-22, CWE-59), and lost-update protection (CWE-367). The shared cases make the IDE and the extension apply the same rules (NIST SI-10).
- **Accepted risk:** Node has no handle-based file access, so the link check and the open are two steps (TOCTOU, CWE-367). Only a program already running as the person could use the gap, and it can change the files directly. TODO(security) in `workspace.ts`.
- **Verification:** HistoryTest `2026-11-22_113321Z_core_67b1744` (core 104 + 7 passed), `2026-11-22_113334Z_vscode-ext-unit_67b1744` (25 of 25, including links refused, conflicts and budgets), `2026-11-22_113346Z_vscode-ext-integration_67b1744` and `2026-11-22_113352Z_vscode-ext-integration_67b1744` (real VS Code 1.147.0 and 1.95.0: `.env` refused and never sent, nothing written before Accept, a hand edit not overwritten), `2026-11-22_113405Z_vscode-ext-audit_67b1744` (npm audit: 0).

## 2026-11-22 — Side track X1: VS Code extension; IPv6 gateway URL check
- **Type:** hardening (new component); patch (URL check)
- **CVE / CWE / advisory:** none. CWE-20, CWE-79, CWE-113, CWE-209, CWE-295, CWE-319, CWE-400, CWE-522, CWE-601, CWE-668; GHSA-g3hv-75wg-m249 (minimatch, dev tool only, overridden)
- **Severity:** URL check: low (only loopback IPv6 addresses could be affected, and the extra text was dropped, not used). KEV: no.
- **Affected:** `vscode-extension/` (new), `core/src/gateways.rs` (`split_host_port`)
- **What changed:** the extension keeps keys only in VS Code's SecretStorage and sends them only as `Authorization` to a strictly checked https address (plain http only to loopback, marked INSECURE — DEV ONLY). It never follows redirects, supports an optional pinned CA, and uses a webview with `default-src 'none'`, nonce scripts and text-only rendering. Every answer and webview message has size caps, errors are fixed messages, and nothing is exported to other extensions. Its test hooks exist only in VS Code's test mode. The Rust URL check now refuses text after an IPv6 `]`.
- **Why it is secure now:** the same allow-list checks hold in both clients, enforced by the shared test cases (NIST SI-10, SC-8, SC-13; ASVS V5, V6.4, V9).
- **Accepted risk:** VS Code may send extension requests through its configured proxy and add OS certificates to the trust list, so a pinned CA may not be the only root inside VS Code (TODO(security) in `client.ts`). `minimatch` is overridden to 11.0.9 in the dev-only packager `@vscode/vsce` 4.1.1, which asks for ^10.2.6. Packaging works with it.
- **Verification:** HistoryTest `2026-11-22_112152Z_core_a704618` (core 99 + 5 passed, with the shared cases), `2026-11-22_112214Z_vscode-ext-unit_a704618` (extension 14 of 14), `2026-11-22_112214Z_vscode-ext-integration_a704618` (real VS Code 1.147.0: pass), `2026-11-22_112220Z_vscode-ext-integration_a704618` (real VS Code 1.95.0: pass), `2026-11-22_112222Z_vscode-ext-audit_a704618` (npm audit: 0), `2026-11-22_112223Z_vscode-ext-package_a704618` (.vsix built).

## 2026-11-22 — url 2.5.9 (RUSTSEC-2026-0592)
- **Type:** CVE remediation (dependency patch)
- **CVE / CWE / advisory:** RUSTSEC-2026-0592, stack overflow in `Url::origin()` on nested `blob:` URLs (CWE-674, uncontrolled recursion). No CVE ID was listed in the advisory output we saw.
- **Severity:** denial of service (crash) at most. No CVSS score was shown in `cargo audit`. KEV: no.
- **Affected:** `app/src-tauri/Cargo.lock` (`url` 2.5.8, pulled in by Tauri and HTTP crates); `core/Cargo.lock` was not affected.
- **What changed:** `cargo update -p url --precise 2.5.9`, which also updated `form_urlencoded` 1.2.2 to 1.2.3.
- **Why it is secure now:** the patched version from the advisory is in the lockfile (NIST SI-2, flaw remediation).
- **Verification:** `cargo audit` in `app/src-tauri`: exit 0 with only the unmaintained and unsound warnings that are already allowed. `cargo check` passes. Found by PC local CI (`local-ci/supply-chain`) on PR 23. HistoryTest 2026-11-22_101242Z_security-scan_9ebb059.

## 2026-11-22 — Phase I6c-4: Node host pins moved to our own build
- **Type:** hardening (supply chain)
- **CVE / CWE / advisory:** none. CWE-494; NIST SI-7, SA-12
- **Severity:** none. KEV no.
- **Affected:** `core/src/node_host.rs` (`PINS`, `SERVER_BUILD`, `SERVER_COMMIT`, `VSCODE_VERSION`), `core/tests/node_host_real.rs`
- **What changed:** the pins are our Linux x64 and Windows x64 builds of VS Code 1.145.2's server, on the IDE repo's pre-release `node-host-1.145.2` (github.com, already an allowed host). The VSCodium 1.140 pins were removed. macOS and Linux arm64 have no pin, so they get nothing (fail closed). On Windows the build script adds the Windows SDK's tools folder (a fixed admin-only path, CWE-427) to the end of `PATH` for `signtool.exe`, which VS Code uses only to check and strip signatures. The repo was made public on Pheonix's choice; before that, its full history was scanned for secret patterns (tokens, keys, private keys) and only test fixtures matched. Commit author emails become public, which Pheonix accepted.
- **Why it is secure now:** the app accepts only the exact published bytes. The pin was taken by downloading the file back anonymously from the release and hashing it, and it equals the build record's hash (NIST SI-7, CWE-494).
- **Accepted risk:** the publisher's GitHub account can replace a release file, but the app would refuse it because of the pinned hash. gitleaks itself was not run (not installed in the cloud); the regex scan is weaker.
- **Verification:** HistoryTest 2026-11-22_061544Z_node-host-pin_dab272f (published file downloaded anonymously, SHA-256 equals the pin, pass), 2026-11-22_061550Z_core_dab272f (core tests 95 + 5 + 2 passed), 2026-11-22_061612Z_ui_dab272f (UI smoke 114/114). The real download test (`node_host_real`, ignored by default) can't run in the cloud because the app uses no proxy and the cloud has no direct internet; it runs on the PC once the Windows pin is in.

## 2026-11-22 — Phase I6c-4: Node extension host server built from pinned VS Code source
- **Type:** hardening (supply chain)
- **CVE / CWE / advisory:** none. CWE-494, CWE-20, CWE-78; NIST SA-12, CM-7, SI-7; SSDF PS.2, PW.4; SLSA build L1
- **Severity:** none (new build tooling). KEV no.
- **Affected:** `scripts/build_node_host.py`, `core/examples/node_host_serve.rs`, `app/src/monaco.ts`, `app/tests/ui-smoke.mjs`
- **What changed:** the server for Node extensions can be built from VS Code's MIT source at an exact tag and commit (checked twice: the clone and the built `product.json`), with Node checked against the tag's `.nvmrc` and dependencies from the lockfile (`npm ci`). The GitHub Copilot extension and runtime and the marketplace debugger extensions are removed from the server (least functionality, NIST CM-7: no third-party AI client in the IDE's server). Only two files in the clone are changed, and the build stops on any other change. The archive gets a SHA-256 and a build record for pinning.
- **Why it is secure now:** the bytes the app will pin come from a known source commit, not from a third party's binary release; the app keeps refusing any other bytes (NIST SI-7).
- **Accepted risk:** builds aren't bit-for-bit reproducible across machines yet (VS Code's bundler); the pin is of the published file. The cloud build filled `@vscode/ripgrep`'s cache by hand from the same github.com release file because the cloud proxy blocks `api.github.com` (that package checks no hash itself, upstream behaviour).
- **Dependency:** VS Code 1.145.2 (MIT), commit c4453f8ea430795f941c1b2ff6c9321135a74a02; Node 24.22.1 (SHA-256 checked against nodejs.org SHASUMS256.txt). A live advisory lookup for VS Code 1.145.2 was not done from the cloud; none known to us on 2026-11-22.
- **Verification:** HistoryTest runs 2026-11-22_053759Z_node-host-build_4096b2e (build, pass) and 2026-11-22_053720Z_ui-node-host-real_832a0a7 (real editor with this server, 116/116).

## 2026-11-22 — Phase I6c-3: no console windows, local folders only, Node host version check
- **Type:** hardening; patch
- **CVE / CWE / advisory:** none. CWE-1188 (unsafe default), CWE-20, CWE-436 (interpretation conflict between editor and server); NIST SI-10, CM-7
- **Severity:** none known to be exploitable (found in testing on the PC). KEV no.
- **Affected:** `core/src/{node_host,container}.rs`, `app/src-tauri/src/nodehost.rs`, `app/src/{nodeHost,ipc}.ts`, `app/src/lib/vscodeVersion.ts`, `app/src/components/NodeHostPanel.tsx`
- **What changed:** console programs started by the windowed app (the Node host's `node.exe`, container engine calls) use `CREATE_NO_WINDOW`, so no stray console window can be typed into or closed by mistake. The folder path sent to the editor drops the `\\?\` prefix and network shares are refused before anything is downloaded or started. The editor checks that the Node host is built from its own VS Code minor version before offering or connecting it; a value that doesn't parse never matches (fail closed).
- **Why it is secure now:** a mismatched server no longer gets a half-working connection whose messages reach the wrong handlers (CWE-436); the check is a deny-by-default allow-list on a strict version format (CWE-20, CWE-1188, NIST SI-10). Node code still runs only in a trusted local folder (NIST AC-3).
- **Verification:** HistoryTest runs 2026-11-22_042952Z_node-host_c727219 (PC, partial: version check verified in the real app, no Node extension ran), 2026-11-22_042549Z_core_c727219 (95 passed), 2026-11-22_042600Z_app-unit_c727219 (8 passed), 2026-11-22_042600Z_ui_c727219 (114/114).

## 2026-11-22 — Phase I6c-3: real VSCodium server pinned by SHA-256
- **Type:** hardening
- **CVE / CWE / advisory:** none. CWE-494, CWE-306; NIST SI-7, SA-12; SSDF PS.2
- **Severity:** none (pins for an existing control). KEV no.
- **Affected:** `core/src/node_host.rs` (`VSCODIUM`, `VSCODIUM_COMMIT`, `PINS`), `core/tests/node_host_real.rs`
- **What changed:** the pin list, empty until now (so nothing could download), holds VSCodium 1.140.06880's server for five OS and CPU pairs. Each hash was computed on the PC from the downloaded file and matched the `.sha256` VSCodium publishes; a file with any other hash is refused. Other platforms still get nothing.
- **Why it is secure now:** only those exact bytes are ever unpacked and run (NIST SI-7, CWE-494). The real server was checked to refuse requests without its connection token (CWE-306) and to close its port when stopped.
- **Dependency:** VSCodium server 1.140.06880 (MIT, built from VS Code 1.140). A live advisory lookup for it was not possible from the cloud; no advisory known to us on 2026-11-22.
- **Verification:** HistoryTest runs 2026-11-22_015908Z_node-host_506f165 (PC, real server, pass) and 2026-11-22_015916Z_core_506f165 (94 passed).

## 2026-11-22 — Phase I6c-2: Node extensions in the editor, local host connection, CSP for 127.0.0.1
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-59, CWE-200, CWE-306, CWE-522, CWE-598, CWE-610, CWE-829, CWE-918; NIST AC-3, SC-7
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{extensions,node_host}.rs`, `app/src-tauri/src/{nodehost,ext}.rs`, `app/src-tauri/tauri.conf.json`, `app/vite.config.ts`, `app/src/{nodeHost,monaco,main,ipc}.ts(x)`, `app/src/components/{NodeHostPanel,ExtensionsView,Editor}.tsx`
- **What changed:** Node extensions install and run only in the local host of a trusted folder; the host gets a fresh copy of the enabled Node extensions on every start (no links). The page may connect to `ws://`/`http://127.0.0.1:*` for the host; the build strips every local source from VS Code's extension host iframe CSP and fails if one is left. The editor sends the token only to the host on 127.0.0.1, over the socket, kept in memory (not the URL or web storage). Extensions can't make the window open other server folders. The full folder path reaches the webview only once the folder is trusted.
- **Why it is secure now:** Node code runs only after the person's trust click (CWE-829, NIST AC-3); web extensions still reach nothing on this computer (CWE-918, NIST SC-7; tested with a counting local server); the host is reachable only with its token (CWE-306).
- **Accepted risk:** while the host runs, the editor and web extensions can read files through it (`vscode-remote://`), including outside the folder; only in a trusted folder, where Node extensions already can. Documented in docs/features/node-extensions-design.md.
- **Dependency:** `@codingame/monaco-vscode-remote-agent-service-override` 39.0.5 (MIT); npm audit gate passed 2026-11-22.
- **Verification:** HistoryTest runs 2026-11-22_014615Z_core_10da8d4 (94 passed), 2026-11-22_014626Z_ui_10da8d4 (113/113), 2026-11-22_014647Z_app-unit_10da8d4, 2026-11-22_014648Z_security-scan_10da8d4.

## 2026-11-21 — Phase I6c-1: Node extension host core (download check, safe unpack, folder trust, local sidecar)
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-22, CWE-59, CWE-214, CWE-306, CWE-330, CWE-409, CWE-494, CWE-522, CWE-532, CWE-601, CWE-829, CWE-918; NIST SI-7, AC-3, AC-6, AU-2; SSDF PS.2
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{node_host,lib}.rs`, `core/Cargo.toml`, `app/src-tauri/src/{nodehost,main}.rs`, both `Cargo.lock` files, `THIRD-PARTY-NOTICES.txt`
- **What changed:** the core can download the VSCodium server only from a per-OS pin with its SHA-256 (the pin list is empty, so it fails closed until the real files are pinned), from GitHub release hosts only, public addresses only, every redirect checked. The `.tar.gz` is unpacked entry by entry into a fresh folder (no `..`, absolute or drive paths, no links, hard links or devices, size and count caps, owner-only permissions) and renamed into place. The server starts only for a folder in the trust list, on 127.0.0.1 with a 192-bit token passed in a 0600 file, with an environment rebuilt from an allow-list so gateway keys and tokens never reach extensions. One server at most; it stops on folder change, trust removal and app exit.
- **Why it is secure now:** a tampered download is never used (NIST SI-7, CWE-494); a hostile archive can't write outside its folder (CWE-22, CWE-59, CWE-409); an untrusted folder never runs Node code (CWE-829); other local programs can't drive the server without the token (CWE-306).
- **Dependency:** `tar` 0.4.46 and `filetime` 0.2.30 added; RustSec scan of both lockfiles found no vulnerabilities (2026-11-21).
- **Accepted risk:** Node extensions run with the person's own rights, as in VS Code; mitigated by folder trust and installs from Open VSX by the person's action only.
- **Verification:** HistoryTest runs 2026-11-21_232857Z_core_3e0d986 (93 passed), 2026-11-21_232914Z_security-scan_3e0d986 (failed: stale notices), 2026-11-21_232948Z_security-scan_f363048 (pass).

## 2026-11-21 — Phase I6b: internet for web extensions through the core, asked per extension
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-113, CWE-345, CWE-400, CWE-532, CWE-565, CWE-601, CWE-862, CWE-918; NIST AC-3, SC-7, SC-8, AU-2
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{ext_net,extensions,lib}.rs`, `app/src-tauri/src/{ext,main}.rs`, `app/src/{extNetShim,extHostWorker,extNet,extensions,monaco,ipc,App}.ts(x)`, `app/src/components/{ExtNetConsent,ExtensionsView}.tsx`
- **What changed:** web extensions may reach public https sites only after the person allows it once per extension (consent card naming the site; Extensions view shows, allows and stops it). Their `fetch` calls go over a private BroadcastChannel to the page and are made by the Rust core, which accepts public https hosts only (checked in the URL and again at connect time), checks every redirect (at most 3), allow-lists methods and headers, drops `Cookie`/`Host`/proxy/hop-by-hop headers, never returns `Set-Cookie`, caps sizes and time, runs at most 8 at once and logs only the extension id and host. The worker's CSP is unchanged.
- **Why it is secure now:** internet is deny by default, granted only by the person's click and checked again by the core on every request (NIST AC-3, CWE-862); an allowed extension still can't reach this computer, the local network, the gateway, TandemForge or cloud metadata (CWE-918, NIST SC-7).
- **Accepted risk:** all web extensions share one worker, so a hostile extension could borrow the access of one the person allowed (by having it make the call, or by forging stack traces through `Error.prepareStackTrace`, which VS Code lets extensions set). The docs say to install only trusted extensions while any has internet. An allowed extension can send what it sees in the editor to the sites it reaches.
- **Verification:** HistoryTest runs 2026-11-21_231802Z_core_caf40d4 (87 passed), 2026-11-21_231813Z_ui_caf40d4 (105/105, including a forged channel and a no-extension-frame call), 2026-11-21_231829Z_app-unit_caf40d4, 2026-11-21_231829Z_security-scan_caf40d4 (no vulnerabilities).

## 2026-11-21 — Phase I6b hardening and accepted risk: web extensions in a sandboxed worker
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-94, CWE-95, CWE-201, CWE-693, CWE-918, CWE-1021; NIST SC-7, AC-4, CM-7; ASVS V14.4
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/extensions.rs`, `app/src-tauri/src/ext.rs`, `app/src/{extensions,monaco}.ts`, `app/src/components/ExtensionsView.tsx`, `app/src-tauri/tauri.conf.json`, `app/vite.config.ts`
- **What changed:** web extensions (a `browser` entry) install after the same hash and signature checks and run only in VS Code's web worker extension host: a Worker created by a hidden iframe (`sandbox="allow-scripts allow-same-origin"`) of the app's own origin. Node-only extensions and extensions asking for proposed APIs are refused. The app CSP gained `'unsafe-eval'` (VS Code loads web extension code with `new Function`), the iframe script's SHA-256 hash, `frame-src 'self'` and `frame-ancestors 'self'` (was `'none'`). The IDE page gets a second, stricter meta CSP without `'unsafe-eval'`, built from the same string. Turning off or removing a web extension restarts the host without it.
- **Why it is secure now:** the worker is bound by both the app CSP and the iframe's own CSP, so it can fetch only `'self'` and `data:`; it has no DOM and no Tauri bridge; `vscode.env.openExternal` is refused. Each was proven from inside a running extension in the UI smoke test (NIST SC-7, AC-4). The page keeps its no-eval policy (CWE-95). Other sites still can't frame the app (CWE-1021).
- **Accepted risk:** `'unsafe-eval'` in the header CSP (the meta CSP removes it for the page; the iframe and its worker keep it). A web extension can read and change the text of files open in the editor, like any editor extension, and can fetch the app's own bundled files. The iframe hash pins monaco-vscode-api 39.0.5. The real desktop webviews (WebView2, WebKitGTK) are not yet checked; that is on the PC list.
- **Verification:** HistoryTest runs 2026-11-21_215005Z_core_fd65ba7 (83 passed), 2026-11-21_215016Z_ui_fd65ba7 (98/98), 2026-11-21_215028Z_app-unit_fd65ba7, 2026-11-21_215028Z_security-scan_fd65ba7 (no vulnerabilities).

## 2026-11-21 — Phase I7 hardening: pairing with TandemForge on this computer
- **Type:** hardening
- **CVE / CWE / advisory:** none (new feature). CWE-20, CWE-209, CWE-400, CWE-522, CWE-532, CWE-918; NIST SC-7, SC-8, IA-2, AC-4
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{tandemforge,gateways,client,lib}.rs`, `app/src-tauri/src/{net,main}.rs`, `app/src/ipc.ts`, `app/src/components/{NetworkView,TandemForgeConnect}.tsx`
- **What changed:** the IDE reads only a port from TandemForge's `engine.json` (4 KiB cap, strict JSON, `api` 1, port 1024 to 65535) and talks only to `http://127.0.0.1:<port>`; a saved TandemForge entry with any other URL is refused when the list loads. The pairing code (`tfa_` + 40 to 100 base64url characters) is checked against TandemForge before it is saved, kept only in the OS keychain, and sent only as a bearer header to loopback. The HTTP client's https-only switch is now off only for loopback http URLs the URL check already allowed.
- **Why it is secure now:** the code never leaves the computer or reaches the webview, logs or files (NIST SC-7, IA-5; CWE-522, CWE-532); TandemForge on its side limits the code to its `/v1` engine routes and local models (TandemForge SECURITY-LOG, same date).
- **Verification:** HistoryTest runs 2026-11-21_205545Z_core_1e284ce, 2026-11-21_205557Z_ui_1e284ce (91/91), 2026-11-21_205630Z_tf-engine-e2e_1e284ce (wrong code 401, pairing code refused on `/api`, browser Origin 403, revoke closes access), 2026-11-21_205639Z_security-scan_1e284ce (no vulnerabilities; carried-over warnings only).

## 2026-11-21 — Phase I6a hardening: verified Open VSX extensions, editor services, CSP
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-22, CWE-59, CWE-79, CWE-94, CWE-117, CWE-209, CWE-345, CWE-400, CWE-409, CWE-494, CWE-601, CWE-918; NIST SI-7, AU-2, AU-3, AC-3, CM-7; SSDF PS.2
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/extensions.rs`, `app/src-tauri/src/{ext,main}.rs`, `app/src/{extensions,monaco,main,ipc}.ts`, `app/src/components/ExtensionsView.tsx`, `app/src-tauri/tauri.conf.json`, `app/package.json`, `app/stubs/jschardet`
- **What changed:** extensions install only on the person's click, one at a time, after the `.vsix` matches the SHA-256 Open VSX publishes and its Ed25519 signature verifies with the key Open VSX publishes; unsigned extensions and any with code (`main`/`browser`) are refused. Downloads are https only, public addresses only, no environment proxy, at most one redirect to the two Open VSX hosts, with size caps. The archive is unpacked entry by entry (only `extension/`, allow-listed path parts, no links, 10,000 files / 50 MiB each / 300 MiB total) through cap-std into a staging folder that is renamed into place. The webview gets files only for enabled extensions and only from their own folder. CSP: `'wasm-unsafe-eval'` added to `script-src` (WebAssembly compilation for the TextMate regex engine; JavaScript `eval` stays blocked), `'self' data: blob:` to `connect-src` and `blob:` to `img-src`/`font-src` for files made inside the page; no remote host added. `jschardet` (LGPL-2.1+) replaced by a stand-in.
- **Why it is secure now:** a swapped download, a package under a borrowed name, extension code, zip slip, zip bombs, SSRF and markup from the registry are each refused before anything is written or shown (NIST SI-7, CM-7; OWASP ASVS V5, V12, V14.4).
- **Accepted risk:** Open VSX is trusted to publish the right hash and key; a compromise of open-vsx.org itself would pass the check (the verified-publisher badge and the person's choice remain). `'wasm-unsafe-eval'` lets any script already allowed by `script-src 'self'` compile WebAssembly. `cargo audit` warnings carried over from before (Tauri's Linux GTK stack: glib 0.18 unsound iterator; chrono, proc-macro-error, zbus_names, zvariant unmaintained), no vulnerabilities.
- **Verification:** HistoryTest runs 2026-11-21_173347Z_core_848b60e, 2026-11-21_173357Z_ovsx-cached_848b60e, 2026-11-21_173412Z_ui_848b60e (87/87), 2026-11-21_173418Z_app-unit_848b60e, 2026-11-21_173522Z_security-scan_848b60e.

## 2026-11-17 — picomatch ReDoS in build tooling (GHSA-fc5q-39xc-vjpf), no fixed version yet
- **Type:** accepted risk
- **CVE / CWE / advisory:** CVE-2026-112366, GHSA-fc5q-39xc-vjpf, CWE-1333 (inefficient regular expression)
- **Severity:** CVSS 3.1 7.5 High (`AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H`); CVSS 4.0 `AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:N/VA:H/SC:N/SI:N/SA:N`.
  EPSS 0.00335 (27th percentile, FIRST, 2026-11-17). CISA KEV: not listed (checked 2026-11-17).
- **Affected:** `picomatch` up to and including 4.0.7 (every published version; 4.0.7 is the newest on npm as of
  2026-11-17). Reached only through dev dependencies in `app/` (`vite` 6 and `@tailwindcss/vite`, through `tinyglobby` and `fdir`). `npm audit --omit=dev` reports 0 vulnerabilities,
  so nothing shipped in the installers contains it.
- **What changed:** no dependency change: there is no patched release to move to (upstream fix proposed in
  micromatch/picomatch PR 194, not released), and npm's suggested "fix" is a vite downgrade that still pulls
  picomatch. The owner chose a dated exception: CI and the local CI now run `scripts/npm_audit_gate.py`, which fails
  on any high or critical advisory in shipped dependencies, and in dev dependencies unless the exact advisory is
  listed, unexpired, in `app/npm-audit-exceptions.json` (this one expires 2026-12-31).
- **Why the risk is accepted:** the flaw is a CPU-exhaustion (availability) issue when an attacker controls a glob
  pattern such as `+(*)`. In this repository picomatch runs only inside build and lint tools on a developer or CI
  machine, with glob patterns written in our own config files; no user, network peer or model output supplies a
  pattern, and none of it runs in the shipped app (NIST RA-3, RA-5; SSDF RV.2). Worst case is a slow or hung local
  build, which the CI timeout stops.
- **Revisit:** when picomatch publishes a release after 4.0.7, update the lock file (`npm update picomatch` or an
  `overrides` entry) and close this entry. Review again by 2026-12-31 at the latest.
- **Verification:** live checks on 2026-11-17: `npm view picomatch versions` (newest 4.0.7), OSV.dev record
  GHSA-fc5q-39xc-vjpf (`last_affected: 4.0.7`), `npm audit --audit-level=high` (fails on this advisory only) and
  `npm audit --omit=dev --audit-level=high` (0 vulnerabilities). HistoryTest run 2026-11-17_232934Z_security-scan_64acea3.

## 2026-11-17 — Phase I5d hardening: local container runner (Docker or Podman)
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-78, CWE-88, CWE-20, CWE-200, CWE-250, CWE-269, CWE-284, CWE-400, CWE-426, CWE-494, CWE-522, CWE-526, CWE-770; NIST AC-6, SC-7, SI-7, SR-3; CIS Docker 5.3, 5.10, 5.12, 5.25, 5.28
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{container,localbox,lib}.rs`, `app/src-tauri/src/{run,agent,main}.rs`, `app/src/components/{RunView,ChatView}.tsx`, `app/src/{App.tsx,ipc.ts}`
- **What changed:** runs go into `<engine> run --rm --pull=never --network=none --cap-drop=ALL
  --security-opt=no-new-privileges --read-only` with a small `nosuid,nodev` `/tmp`, process, memory, open-file and
  core limits, a non-root user (own uid on Linux), only the job folder and the cache mounted (`:Z` on SELinux), a
  fixed environment, and a wall-clock kill by container name. Images come from a fixed list pinned by sha256 digest
  and are downloaded or built only by **Get image**; the engine is found in absolute PATH folders only; all engine
  calls are argument lists; the image id from the webview is checked against the list.
- **Why it is secure now:** project code in a container cannot reach the network, the person's files or secrets,
  change the image, gain privileges or outlive its run; a swapped or tampered image tag cannot change what runs
  (NIST AC-6, SC-7, SI-7, SR-3; SLSA).
- **Accepted risk:** a container shares the host kernel; a kernel or engine escape is out of scope (rootless Podman
  recommended). The Rust image's packages are not pinned one by one (`dnf install rust-toolset`); the base is.
  Image contents are Red Hat's and were not scanned here (`trivy image <ref>` recommended).
- **Verification:** HistoryTest runs 2026-11-17_140846Z_windows-containers_8bba2b4 (PC,
  Docker Desktop: 12/12 real container checks incl. the Rust image build, core tests pass),
  2026-11-17_140706Z_local-containers_104f232 (cloud Docker: 11 passed, Rust skipped for lack of network),
  2026-11-17_140653Z_core_104f232 (unit tests incl. the forbidden-flag check), 2026-11-17_140753Z_security-scan_104f232
  (0 vulnerabilities).

## 2026-11-17 — Phase I5c hardening: Windows local sandbox (AppContainer and job object)
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-78, CWE-88, CWE-158, CWE-200, CWE-403, CWE-404, CWE-426, CWE-459, CWE-526, CWE-732, CWE-400; NIST AC-6, SC-7, SC-4, SI-6
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/localbox.rs`, `core/src/localbox_win.rs`, `core/Cargo.toml`
- **What changed:** Windows runs start suspended in an AppContainer with no capabilities and are resumed only inside
  a job object (kill on close, 1024 processes, 8 GiB, CPU = 8 x time limit, all UI restrictions); only the three
  pipe handles are inherited; the environment is built from scratch; only `.exe` programs found by name on PATH
  (no `.bat`/`.cmd`, no Store aliases); command lines quoted by the C runtime rules, NUL refused; folder grants only
  to the sandbox's own `S-1-15-2-` SID through `icacls` (argument list), recorded before they are made and removed
  after the run or at the next run after a crash; the container's folder emptied after each run; the runner is
  offered only when a confined test start works.
- **Why it is secure now:** project code on Windows cannot reach the network, the person's other files, other
  programs' windows or clipboard, or outlive its run (NIST AC-6, SC-7, SC-4; fail closed per SI-6).
- **Accepted risk (toolchain mode, pending Pheonix's choice):** the job may start other programs (including
  `cmd.exe`) and run files it builds in its job folder and cache; during a run the toolchain folders it uses carry
  a read-and-execute entry for the sandbox SID; the Windows and Program Files folders are readable by any app
  container. Documented in features/local-sandbox.md section 6.
- **Verification:** HistoryTest runs 2026-11-17_134640Z_windows-sandbox_dbbc5cd (real Windows confinement test
  passed; cargo, go, unittest inside the sandbox; npm blocked by the Node.js folder's permissions),
  2026-11-17_133400Z and 134223Z (earlier failing runs), 2026-11-17_134717Z_security-scan_dbbc5cd (0 vulnerabilities).

## 2026-11-17 — Phase I5c hardening: local OS sandbox for runs on this computer
- **Type:** hardening; accepted risk
- **CVE / CWE / advisory:** none (new feature). CWE-200, CWE-522, CWE-22, CWE-59, CWE-269, CWE-250, CWE-693, CWE-400, CWE-404, CWE-426, CWE-526, CWE-668, CWE-78; NIST AC-3, AC-6, SC-4, SC-7, SI-6
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{localbox,testparse,store}.rs`, `app/src-tauri/src/{run,agent,main}.rs`, `app/src/components/{RunView,ChatView}.tsx`
- **What changed:** local runs execute on a fresh owner-only copy of the folder (deny list, no links, caps) inside
  new user, network, PID, mount, IPC, UTS and cgroup namespaces; Landlock allows only system and toolchain folders
  (read and execute) and the job folder and cache (write), and denies TCP; seccomp refuses namespace, mount, ptrace,
  bpf, io_uring, keyring and similar calls, Unix sockets and TIOCSTI, and kills other architectures; no_new_privs,
  rlimits, PID-1 job with parent-death signal, wall-clock kill; cleared environment; program only from PATH folders
  outside the project. Any failed setup step exits 126 before the program starts; the runner is offered only when
  Landlock and user namespaces work.
- **Why it is secure now:** untrusted project code cannot read the person's secrets, change their files, reach the
  network or local services, or outlive the run (NIST AC-6, SC-7, SC-4; fail closed per NIST SI-6).
- **Accepted risk (pending Pheonix's choice):** toolchain mode lets the job execute any program in system and
  toolchain folders, including shells; `/etc` is readable; a kernel bug in namespaces, Landlock or seccomp is out of
  scope. Documented in features/local-sandbox.md section 6.
- **Verification:** HistoryTest runs 2026-11-17_131847Z_core_e980a8c (real-kernel confinement test),
  2026-11-17_131900Z_local-toolchains_e980a8c (6/6), 2026-11-17_131928Z_ui-smoke_e980a8c (77/77),
  2026-11-17_131937Z_security-scan_e980a8c (0 vulnerabilities).

## 2026-11-17 — Phase I5b hardening: running tests and commands, and the assistant asking to run
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-78, CWE-88, CWE-22, CWE-200, CWE-400, CWE-79, CWE-150, CWE-639; OWASP LLM01, LLM06 (none: new feature)
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{runner,agent,gwapi}.rs`, `app/src-tauri/src/{run,agent,missions,main}.rs`, `app/src/components/{RunView,ChatView}.tsx`
- **What changed:** runs go to the gateway's sandbox tools only (no network, read-only, non-root, on a snapshot);
  requests are an argument list or a framework from a fixed list, with checked folder, word count, size, control
  characters and time limit; the chat's run tools exist only with Runs on and a linked folder, and each request
  waits for the person's Allow on a card, answered by its own random id (Decline on Stop, close or 5 minutes);
  output is cleaned and capped; only the gateway runner is accepted; audit words only.
- **Why it is secure now:** a prompt-injected model cannot run anything without the person seeing the exact request
  and allowing it (OWASP LLM06, NIST AC-3, AC-6); nothing runs through a shell (CWE-78); runs cannot reach the
  network (NIST SC-7).
- **Accepted risk:** the folder (minus the deny list) is sent to the linked gateway, as in phase I4; a person-started
  run needs no second confirmation. Documented in features/run-and-test.md section 6.
- **Verification:** HistoryTest runs 2026-11-17_125930Z_core_618354d, 2026-11-17_125934Z_ui-smoke_618354d (75/75),
  2026-11-17_125953Z_live-gateway_618354d (13/13: no network, time limit, `.env` never stored),
  2026-11-17_125944Z_security-scan_618354d (0 vulnerabilities).

## 2026-11-17 — Installers: licence gate and signed checksums
- **Type:** hardening (supply chain); accepted risk
- **CVE / CWE / advisory:** none. CWE-494, CWE-1357; NIST SI-7, SR-3, AC-6; SLSA build integrity
- **Severity:** none (new release path)
- **Affected:** `app/src-tauri/tauri.conf.json`, `app/src-tauri/THIRD-PARTY-NOTICES.txt`
- **What changed:** installers are built by P2P-AI's `release_app.py` from a clean tagged tree, after a deny-by-default
  licence gate; `SHA256SUMS` is cosign-signed; uploads are drafts. Per-user NSIS install (no admin).
- **Accepted risk (Pheonix, 2026-11-17):** no Authenticode signature yet; Windows shows a SmartScreen warning and users
  check `SHA256SUMS` with the project key.
- **Why it is secure now:** downloads can be verified with the project key (NIST SI-7); licences are gated (SR-3).
- **Verification:** [licence gate](../HistoryTest/runs/2026-11-17_085507Z_installers_1b95d91/RUN.md), [Linux bundle](../HistoryTest/runs/2026-11-17_085509Z_installers_1b95d91/RUN.md).

## 2026-11-17 — Workspace file access through a folder handle (TOCTOU)
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-367, CWE-59, CWE-22 (none: no reported vulnerability)
- **Severity:** none scored; it needed another local program racing the IDE. KEV no.
- **Affected:** `core/src/workspace.rs`, `core/Cargo.toml`, `core/Cargo.lock`, `app/src-tauri/Cargo.lock`
- **What changed:** the workspace keeps a cap-std `Dir` handle on the opened folder. Paths still pass the allow-list,
  then are opened beneath that handle, which refuses any path (including through links) that leaves the folder; type
  and size come from the open file. Saves open the parent folder once and write the temp file and rename inside it.
- **Why it is secure now:** no check-then-use on path names remains (CWE-367); confinement is enforced by the open
  itself (NIST AC-3, AC-6). cap-std is a Bytecode Alliance crate used by Wasmtime (NIST SP 800-161: maintained, widely used).
- **Verification:** HistoryTest runs 2026-11-17_125223Z_core_6ebd731 (66 tests incl. the link-swap test), 2026-11-17_125233Z_security-scan_6ebd731 (OSV: 0 advisories), 2026-11-17_125226Z_shell_6ebd731 (shell build
  not possible in the container, marked invalid).

## 2026-11-17 — Phase I5a hardening: running git on repositories someone else made
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-829, CWE-94, CWE-78, CWE-88, CWE-427, CWE-526, CWE-22, CWE-400, CWE-79, CWE-209 (none: new feature)
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/scm.rs`, `core/src/workspace.rs`, `app/src-tauri/src/{scm,main}.rs`, `app/src/components/{SourceControlView,DiffReview}.tsx`
- **What changed:** git runs from an absolute PATH entry outside the folder, with an argument list, a cleared
  environment, `GIT_LITERAL_PATHSPECS=1`, hooks off (`core.hooksPath` to the null device, `--no-verify`),
  `core.fsmonitor=false`, `protocol.allow=never`; repositories whose own config sets filter programs (other than
  git-lfs), signing, includes, `core.worktree` or worktree config are refused; paths from git pass the workspace
  allow-list; time, output and entry caps; fixed messages.
- **Why it is secure now:** opening or committing in an untrusted repository cannot run that repository's programs
  (CISA Secure by Design, NIST CM-7, SI-10); no network or credentials are involved (NIST AC-6).
- **Accepted risk:** the person's global git settings and the git installation are trusted (they are the person's
  own); documented in features/source-control.md section 6.
- **Verification:** HistoryTest runs 2026-11-17_090743Z_core_86e1c4f (real-git test: planted hook did not run,
  literal `*`), 2026-11-17_090746Z_ui-smoke_86e1c4f (64/64), 2026-11-17_090757Z_security-scan_86e1c4f (0 vulnerabilities).

## 2026-11-17 — Phase I4 hardening: project sync and missions with review
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-200, CWE-538, CWE-345, CWE-494, CWE-362, CWE-367, CWE-22, CWE-73, CWE-918, CWE-93, CWE-400, CWE-79, CWE-209, CWE-532; OWASP LLM06 (none: new feature)
- **Severity:** none (new feature built with these controls). KEV no.
- **Affected:** `core/src/{gwapi,projects,agent,workspace,client}.rs`, `app/src-tauri/src/{missions,net,main}.rs`, `app/src/components/{MissionsView,ChatView,StatusBar}.tsx`
- **What changed:** pushing the folder applies the agent deny list, skips symlinks and build folders, re-hashes each
  file just before upload, and commits with `base_version`; pulled files must match the listed SHA-256, must be text,
  never include denied names, and only become pending changes accepted per file; approval decisions carry the
  arguments' SHA-256; gateway paths pass a strict grammar; API calls are limited to `/v1/` on the active gateway;
  multipart names allow-listed with a random boundary; size and count caps everywhere; gateway error text replaced by
  fixed messages; fixed audit words.
- **Why it is secure now:** human approval for every local write (NIST AC-6, OWASP LLM06), integrity of downloaded
  content (NIST SI-7), no lost updates (CWE-362/367), least data sent (NIST SC-8 over TLS, deny list), no
  information leakage in errors or logs (NIST AU-3, CWE-209/532).
- **Accepted risk:** the folder's contents (except denied names) are uploaded to the chosen gateway and copied to a
  node to run the mission; a secret inside an ordinary file is sent. Mitigation: deny list, the skipped-files report,
  choose a trusted gateway, keep secrets out of the code folder (features/missions.md section 6).
- **Verification:** HistoryTest runs 2026-11-17_082336Z_core_7e32c2d (60 tests), 2026-11-17_082340Z_shell_7e32c2d,
  2026-11-17_082340Z_ui-smoke_7e32c2d (57/57), 2026-11-17_082359Z_security-scan_7e32c2d (0 vulnerabilities), 2026-11-17_085225Z_live-gateway_bfae51f (real
  gateway: .env never stored, stale approval digest refused, nothing written before Accept; 19/19).

## 2026-11-17 — Phase I3 hardening: human approval for every agent change
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-200, CWE-538, CWE-22, CWE-367, CWE-20, CWE-400, CWE-79; OWASP LLM01, LLM06 (none: new feature)
- **Severity:** none (new feature built with these controls)
- **Affected:** `core/src/agent.rs`, `core/src/client.rs`, `app/src-tauri/src/agent.rs`, `app/src/components/{ChatView,DiffReview}.tsx`
- **What changed:** the model's tools only read; edits become pending changes written on the person's Accept with the
  version check; likely secrets are hidden from every tool; strict tool arguments; step, call, size and turn budgets;
  tool text shown as text; audit lines without paths.
- **Why it is secure now:** least privilege and human approval for every write (NIST AC-6, OWASP LLM06), prompt
  injection limited to proposals the person reviews (OWASP LLM01), confinement and lost-update checks reused from the
  editor.
- **Accepted risk:** the deny list is by name; secrets inside ordinary files can be read and sent to the chosen
  gateway (documented in features/agent-tools.md section 6, with Tools off as the mitigation).
- **Verification:** HistoryTest runs 2026-11-17_065236Z_core_e919fcd (48 tests), 2026-11-17_065240Z_ui-smoke_e919fcd
  (45/45), 2026-11-17_065249Z_security-scan_e919fcd (0 vulnerabilities).

## 2026-11-17 — App icons built from the official logo
- **Type:** hardening (supply-chain hygiene for binary assets)
- **CVE / CWE / advisory:** none. NIST SSDF PS.3, PW.6.
- **Severity:** not scored (no vulnerability fixed). KEV no.
- **Affected:** `app/src-tauri/icons/*`, `app/src-tauri/tauri.conf.json` (`bundle.icon`).
- **What changed:** the icons are generated from one committed source logo by a script in the P2P-AI repository instead of placeholders; no code, CSP or capability changed.
- **Why it is secure now:** binary assets are reproducible and reviewable (`make_icons.py --check` catches drift), and no new dependency enters this repository.
- **Verification:** `HistoryTest/runs/2026-11-17_062058Z_branding_ea76006` (29 of 29 checks, 11 for the IDE set).
## 2026-11-17 — Phase I2b hardening: signed public gateway list
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-347, CWE-345, CWE-694, CWE-918, CWE-350, CWE-601, CWE-295, CWE-294, CWE-330, CWE-79,
  CWE-400 (none: new feature)
- **Severity:** none (new feature built with these controls)
- **Affected:** `core/src/discovery.rs`, `core/src/client.rs` (public-only agent for kind `public`),
  `app/src-tauri/src/net.rs`, `app/src/components/PublicGateways.tsx`
- **What changed:** adverts are listed only after the same checks as the P2P-AI node (strict JSON, id bound to key,
  ed25519 signature before time, public https URLs, closed `client_access` values); a resolver drops private
  addresses after DNS for directories, probes and every request to a public-list gateway; probe nonces come from the
  OS CSPRNG and replies must be signed; the saved address comes from the verified cache, not the webview; advert text
  is shown as text, links not clickable; caps on every size and count.
- **Why it is secure now:** NIST SC-13 and SC-8, SI-10 (input validation), ASVS V5 (validation) and V12.6 (SSRF);
  the verifier passes the protocol's shared vectors, including all 25 refusal cases.
- **Verification:** HistoryTest runs 2026-11-17_063819Z_core_545bdf4 (41 tests),
  2026-11-17_063823Z_ui-smoke_545bdf4 (37/37), 2026-11-17_063832Z_security-scan_545bdf4 (0 vulnerabilities).

## 2026-11-17 — Phase I2a hardening: gateway keys, TLS and untrusted answers
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-522, CWE-532, CWE-295, CWE-601, CWE-918, CWE-79, CWE-113, CWE-22, CWE-276, CWE-400
- **Severity:** none (new feature built with these controls)
- **Affected:** `core/src/{gateways,client,chats,store}.rs`, `app/src-tauri/src/net.rs`, `app/src/components/{NetworkView,ChatView}.tsx`
- **What changed:** keys only in the OS keychain and never returned to the webview; URL allow-list; TLS always on
  (pinned CA as the only root when set) except a badged loopback dev gateway; no redirects, no environment proxy;
  answers rendered as text only; state files owner-only and re-checked on load; caps on every size and time.
- **Why it is secure now:** NIST IA-5 and SC-8/SC-13, ASVS V9 (communications) and V6 (stored secrets), OWASP LLM01
  (answers treated as untrusted data). See docs/features/gateways-and-chat.md section 3.
- **Verification:** core tests (fake gateway: key in header only, no redirects, status mapping, stream caps, Stop),
  UI smoke test (key never shown, `<img onerror>` in an answer stays text). HistoryTest runs listed in the feature page.

## 2026-11-17 — Accepted risk: zbus 5 helper crates flagged unmaintained (Linux keychain)
- **Type:** accepted risk
- **CVE / CWE / advisory:** RUSTSEC-2026-0370 (zvariant), RUSTSEC-2026-0371 (zbus_names), RUSTSEC-2026-0372
  (zvariant_derive), all "unmaintained", informational; no vulnerability and no CVE.
- **Severity:** no CVSS (informational). KEV no.
- **Affected:** `app/src-tauri/Cargo.lock`, Linux builds only: keyring 4.2.0 -> zbus-secret-service-keyring-store
  1.0.1 -> zbus 5.19.0 -> zvariant 5.15.1, zbus_names 4.3.5.
- **What changed:** nothing. The crates were folded into zbus 6 (its `wire` module) by the same authors; the
  keyring store has not moved to zbus 6 yet. They only encode D-Bus messages to the local Secret Service.
- **Why it is acceptable now:** the D-Bus peer is the user's own session keyring service, not a network party; the
  data is the IDE's own key entries. Windows and macOS builds do not include these crates.
- **Follow-up:** `TODO(security)`: move to a keyring store on zbus 6 when released; cargo audit in CI keeps
  reporting them until then.
- **Verification:** `cargo audit --file app/src-tauri/Cargo.lock`: 0 vulnerabilities, 5 warnings (these 3 plus the
  2 already accepted). HistoryTest run listed in docs/features/gateways-and-chat.md.

## 2026-11-17 — Accepted risk: Object.prototype is not frozen (Monaco needs it writable)
- **Type:** accepted risk
- **CVE / CWE / advisory:** CWE-1321 (prototype pollution); no CVE.
- **Severity:** no score; this is a defence-in-depth setting, not a known vulnerability. KEV no.
- **Affected:** `app/src-tauri/tauri.conf.json` (`app.security.freezePrototype`), phase I1 release builds.
- **What changed:** `freezePrototype` went from `true` to `false`. With it on, Monaco's keyboard code
  (`KeyCodeUtils.toString = ...`) throws "Cannot assign to read only property 'toString'" while the bundle loads, so
  React never mounts and the window stays blank. Found on the Windows PC; the headless UI smoke test missed it
  because it did not apply the freeze.
- **Why it is acceptable now:** the webview runs only the app's own bundled scripts (`script-src 'self'`, no eval,
  no remote sources), so no outside script can pollute prototypes; file text and later model output are rendered as
  text, never run. The Rust commands validate every argument themselves (paths, sizes, versions) and do not trust
  object shapes from the webview. The UI smoke test now applies the freeze whenever the config turns it on, so a
  future Monaco that works with it can be re-enabled safely.
- **Verification:** UI smoke test with the freeze applied: fails with the same TypeError (reproduced); with the
  setting off: 16/16. Windows window check after the fix: see the HistoryTest runs listed in
  `docs/features/editor-shell.md`.

## 2026-11-17 — Accepted risk: two RustSec warnings in Tauri's Linux GTK stack
- **Type:** accepted risk
- **CVE / CWE / advisory:** RUSTSEC-2024-0370 (proc-macro-error unmaintained), RUSTSEC-2024-0429 (glib
  `VariantStrIter` unsoundness). No CVE named by cargo audit.
- **Severity:** warnings, not vulnerabilities (cargo audit 0.22.1, 2026-11-17). CVSS/EPSS not applicable; KEV no.
- **Affected:** `app/src-tauri/Cargo.lock`, pulled in by tauri 2.12.5 through wry and gtk-rs 0.18 on Linux only.
- **What changed:** nothing; the IDE does not call `glib::VariantStrIter`, and proc-macro-error runs only at build
  time. The P2P-AI desktop app has the same pair.
- **Why it is acceptable:** no reachable code path; fixed upstream when Tauri moves off gtk-rs 0.18. CI runs cargo
  audit on every change, so a real vulnerability fails the build.
- **Verification:** `HistoryTest/runs/2026-11-17_045933Z_security-scan_fe528ba`.

## 2026-11-17 — Monaco's vendored DOMPurify replaced by the patched release
- **Type:** CVE remediation (dependency)
- **CVE / CWE / advisory:** GitHub advisories GHSA-p98j-92pf-mc4p, GHSA-6688-9rhm-gjv2, GHSA-cg2g-xh64-v4q7,
  GHSA-mrv2-3pqr-j5jr, GHSA-c6gr-qvv4-5v3f (DOMPurify <= 3.4.16), as reported by `npm audit` on 2026-11-17; CWE-79.
  No CVE IDs were listed by npm audit, so none are named here.
- **Severity:** npm audit rated the dompurify entry critical. CVSS and EPSS not looked up; KEV not checked.
  Reachability in this app is low: Monaco calls DOMPurify with `RETURN_DOM_FRAGMENT` and none of `IN_PLACE`,
  `SAFE_FOR_TEMPLATES` or `PARSER_MEDIA_TYPE` (checked in `monaco-editor/esm/vs/base/browser/domSanitize.js`), which
  the advisories need. Fixed anyway rather than accepted.
- **Affected:** monaco-editor 0.57.0, which pins `dompurify` 3.4.15 and also ships its own copy at
  `esm/vs/base/browser/dompurify/dompurify.js`.
- **What changed:** `app/package.json` adds `dompurify` 3.4.18 and an npm `overrides` entry, and `app/vite.config.ts`
  redirects Monaco's import of its vendored copy to that package, so the bundle contains only 3.4.18.
- **Why it is secure now:** the sanitizer Monaco runs on hover and markdown content is the release with the fixes
  (NIST SI-2 flaw remediation, SSDF RV.2).
- **Verification:** `npm audit` 0 vulnerabilities and `npm ls dompurify` = 3.4.18
  (`HistoryTest/runs/2026-11-17_045808Z_security-scan_fe528ba`); the built bundle has `version=3.4.18` and no
  `3.4.15` (`HistoryTest/runs/2026-11-17_045746Z_webview_fe528ba`); the UI smoke test passes on that bundle
  (`HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`). Remove the redirect once a Monaco release ships 3.4.17+.

## 2026-11-17 — Accepted risk: `style-src 'unsafe-inline'` for Monaco
- **Type:** accepted risk
- **CVE / CWE / advisory:** none; CWE-79 (defence in depth reduced for styles only)
- **Severity:** low. Scripts stay `script-src 'self'` with no inline scripts or eval, so injected markup cannot run code;
  inline styles allow at most visual tricks or CSS-based data probing within the page.
- **Affected:** `app/src-tauri/tauri.conf.json` CSP.
- **What changed:** Monaco creates `<style>` elements and style attributes at run time, which a strict `style-src 'self'`
  blocks (the editor does not render).
- **Why it is acceptable:** all file text is rendered as text by React and Monaco, not as HTML; there is no remote
  content and `connect-src` allows only Tauri's IPC, so CSS cannot send data out (NIST SC-18 partial).
- **Verification:** UI smoke test under the shipped CSP reports no CSP violations
  (`HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`). Revisit with Monaco's nonce support in phase I6.

## 2026-11-17 — Phase I1 controls (hardening)
- **Type:** hardening (new feature with its controls)
- **CVE / CWE / advisory:** none. CWE-22, CWE-59, CWE-73, CWE-367, CWE-400, CWE-1333, CWE-209, CWE-117, CWE-284.
- **Severity:** not scored (no vulnerability fixed).
- **Affected:** `core/src/workspace.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/capabilities/default.json`.
- **What changed:** path allow-list and canonical confinement, link refusal on save, versioned atomic saves, caps,
  plain-text search, folder chosen only through the OS picker in Rust, no plugin permissions for the webview, fixed
  error messages and path-free audit lines. Details: [features/editor-shell.md](features/editor-shell.md) section 3.
- **Why it is secure now:** NIST AC-6, AU-3, SI-10; ASVS V12.3.1, V14.4.
- **Verification:** `HistoryTest/runs/2026-11-17_045740Z_core_fe528ba` (10 unit tests with malicious paths, links,
  sizes and conflicts), `HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`.
