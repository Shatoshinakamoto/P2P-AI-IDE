# P2P-AI-IDE

An agentic code editor for the P2P-AI network: a VS Code-style editor (Monaco) in a Rust + React app (Tauri), where
you chat with the network's agents, review their edits as diffs, and switch Autopilot and Auto Improve on and off.
It is for people using the network, not for volunteers lending computers (that is the P2P-AI desktop app).

**Status:** phase I1, the editor shell: open a folder, browse, search, edit in tabs, save. Gateways, chat, missions and
extensions follow in phases I2 to I6. The plan and its decisions (ADR-029) are in the P2P-AI repository,
`docs/restructure/agentic-ide-plan.md`.

## Build

Needs Rust stable, Node 22 and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
(on Linux: `libwebkit2gtk-4.1-dev`, `librsvg2-dev`).

```
cd app
npm ci --ignore-scripts
npm run tauri dev       # development window
npm run release         # release binary, no installer
```

Tests: `cd core && cargo test`, `cd app && npm test && npm run build && npm run ui-smoke`. Details in
[docs/commands/app-scripts.md](docs/commands/app-scripts.md).

## Layout

| Folder | What it is |
|---|---|
| `core/` | Rust core: every file operation, with path confinement and size caps |
| `app/src-tauri/` | Tauri shell: six typed commands, the folder picker, CSP and permissions |
| `app/src/` | React webview: layout, explorer, search, Monaco editor |
| `docs/` | Documentation ([index](docs/README.md)) |
| `HistoryTest/` | Record of every test run (development only, never shipped) |

## Security

Development follows [CLAUDE.md](CLAUDE.md). The controls are in
[docs/features/editor-shell.md](docs/features/editor-shell.md) section 3; changes go in
[docs/SECURITY-LOG.md](docs/SECURITY-LOG.md).

## Licence

Apache-2.0 (see LICENSE and NOTICE).
