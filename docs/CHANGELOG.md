# Changelog

All notable changes to the P2P-AI IDE. Format: [Keep a Changelog](https://keepachangelog.com), newest first.

## 2026-11-17 - Phase I1: editor shell

Requested by Pheonix (IDE plan, ADR-029 in the P2P-AI repository; mockup approved 2026-11-17).

### Added
- Rust core `p2p-ide-core`: workspace folder with confined paths, folder listing, text reads, atomic saves with a
  SHA-256 version check, plain-text search, with caps on every size.
- Tauri shell with six typed commands and the native folder picker (called from Rust only).
- React webview in a VS Code-style layout: activity bar, explorer, search, Monaco editor tabs with unsaved markers,
  Ctrl+S, conflict banner, status bar; Chat, Missions, Gateways and Extensions shown as coming in later phases.
- CI: core and shell lint and tests, webview build and UI smoke test, npm audit and cargo audit.
- Windows check: the same commit built, tested and launched on a Windows 11 PC (HistoryTest run
  `2026-11-17_051458Z_windows_ba63e5c`).
- `HistoryTest/` and the recorder `scripts/history_test.py` (copied from the P2P-AI repository), `CLAUDE.md`, docs.

### Fixed
- Blank window in release builds: `freezePrototype` is off because Monaco cannot load with a frozen
  `Object.prototype`; the UI smoke test now applies the freeze when the config asks for it.

### Security
- Monaco's vendored DOMPurify 3.4.15 is replaced by the patched 3.4.18 at build time (SECURITY-LOG).
- `style-src 'unsafe-inline'` in the CSP for Monaco (accepted risk, SECURITY-LOG).

### Test evidence
- `HistoryTest/runs/2026-11-17_045740Z_core_fe528ba`, `..._045741Z_shell_fe528ba`, `..._045746Z_webview_fe528ba`,
  `..._045806Z_ui-smoke_fe528ba` (16/16), `..._045808Z_security-scan_fe528ba` (npm audit 0),
  `..._045933Z_security-scan_fe528ba` (cargo audit: 0 vulnerabilities, 2 upstream warnings).
