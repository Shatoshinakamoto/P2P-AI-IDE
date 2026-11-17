# Changelog

All notable changes to the P2P-AI IDE. Format: [Keep a Changelog](https://keepachangelog.com), newest first.

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
