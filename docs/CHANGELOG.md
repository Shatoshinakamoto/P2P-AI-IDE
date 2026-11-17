# Changelog

All notable changes to the P2P-AI IDE. Format: [Keep a Changelog](https://keepachangelog.com), newest first.

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
