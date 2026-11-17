# P2P-AI IDE documentation

The IDE's plan and its decisions (ADR-029) live in the P2P-AI repository: `docs/restructure/agentic-ide-plan.md`.

| Document | Description |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | Every addition, change, fix and removal, newest first |
| [SECURITY-LOG.md](SECURITY-LOG.md) | Security fixes, hardening and accepted risks |
| [features/editor-shell.md](features/editor-shell.md) | Phase I1: open a folder, explorer, Monaco tabs, save with conflict check, search; threat model and controls |
| [features/gateways-and-chat.md](features/gateways-and-chat.md) | Phase I2a: saved gateways (local and manual), keys in the OS keychain, streamed chat, chat history; threat model and controls |
| [features/public-gateways.md](features/public-gateways.md) | Phase I2b: public gateway list from signed directory adverts, verified like the node; threat model and controls |
| [features/agent-tools.md](features/agent-tools.md) | Phase I3: chat tools on the open folder (list, read, search, propose edits) and the diff review; threat model and controls |
| [files/app-data.md](files/app-data.md) | The IDE's own data files: `gateways.json`, `directories.json` and chat history, every key |
| [files/discovery-vectors.md](files/discovery-vectors.md) | The copy of the P2P-AI protocol's shared advert vectors used by the core tests, with provenance |
| [commands/app-scripts.md](commands/app-scripts.md) | The npm scripts and cargo commands used to build and test the app |
| [commands/history-test.md](commands/history-test.md) | The test history recorder `scripts/history_test.py` (copied from the P2P-AI repository) |
| [files/app-icons.md](files/app-icons.md) | The app icons: P2P-AI logo with the IDE badge, how they are built |
| [configuration/tauri-conf.md](configuration/tauri-conf.md) | `tauri.conf.json` (window, CSP) and `capabilities/default.json`, every key |
| [../HistoryTest/README.md](../HistoryTest/README.md) | The record of every test run |
