# Security log

Every security patch, hardening change, CVE remediation and accepted risk, newest first.

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
- **Verification:** HistoryTest runs listed in features/local-sandbox.md section 8 (core with the real-kernel
  confinement test, real toolchains 6/6, UI smoke 77/77, dependency audit).

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
