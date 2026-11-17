# P2P-AI IDE documentation

The IDE's plan and its decisions (ADR-029) live in the P2P-AI repository: `docs/restructure/agentic-ide-plan.md`.

| Document | Description |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | Every addition, change, fix and removal, newest first |
| [SECURITY-LOG.md](SECURITY-LOG.md) | Security fixes, hardening and accepted risks |
| [features/editor-shell.md](features/editor-shell.md) | Phase I1: open a folder, explorer, Monaco tabs, save with conflict check, search; threat model and controls |
| [features/gateways-and-chat.md](features/gateways-and-chat.md) | Phase I2a: saved gateways (local and manual), keys in the OS keychain, streamed chat, chat history; threat model and controls |
| [files/app-data.md](files/app-data.md) | The IDE's own data files: `gateways.json` and chat history, every key |
| [commands/app-scripts.md](commands/app-scripts.md) | The npm scripts and cargo commands used to build and test the app |
| [commands/history-test.md](commands/history-test.md) | The test history recorder `scripts/history_test.py` (copied from the P2P-AI repository) |
| [configuration/tauri-conf.md](configuration/tauri-conf.md) | `tauri.conf.json` (window, CSP) and `capabilities/default.json`, every key |
| [../HistoryTest/README.md](../HistoryTest/README.md) | The record of every test run |
