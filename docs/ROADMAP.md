# P2P-AI IDE roadmap and checklist

The plan and its decisions (ADR-029) live in the P2P-AI repository, `docs/restructure/agentic-ide-plan.md`, section 8.
This page tracks where each phase stands, with the pull request that shipped it. Tick items in the same pull request
that ships them.

Status key: `[ ]` not started · `[~]` in progress or partly done · `[x]` done.
Last checked against `main` (468ee3b): 2026-11-21.

## Phases

- [x] **I0** Clickable mockup for Pheonix's approval (Design canvas, 2026-11-17)
- [x] **I1** Shell: open folder, explorer, Monaco tabs, save with conflict check, search, settings, no network (#1)
- [x] **I2** Gateways: local, manual and public list from signed adverts; keys in the OS keychain; streamed chat with history (#1)
- [x] **I3** Interactive agent: list, read, search and propose-edit tools on the open folder; diff review (#3)
- [x] **I4** Missions: sync to a gateway project, Autopilot and Auto Improve, Missions view, review of returned work (#4, Windows evidence #5)
- [x] **I5** Commands and tests, source control
  - [x] I5a Source Control view (#7)
  - [x] I5b Run and test on the gateway sandbox, per-run approval in chat (#9)
  - [x] I5c Local OS sandbox, Linux (#10) and Windows AppContainer (#11), ported from TandemForge
  - [x] I5d Local container runner, Docker or Podman, pinned UBI images (#12)
- [x] **I6** Extensions (design fixed in ADR-029 D1)
  - [x] I6a VS Code workbench services on Monaco (`@codingame/monaco-vscode-api`) and extension install from Open VSX by explicit user action (#17, 2026-11-21: themes, icon themes, grammars, snippets; [features/extensions.md](features/extensions.md)). Still to do: a live install from open-vsx.org on the PC
  - [x] I6b Web extensions in a Web Worker (2026-11-21, on #18: sandboxed iframe + worker, no Tauri commands; internet only when the person allows it per extension, through the core; [features/extensions.md](features/extensions.md#web-extensions-i6b)). Still to do: check in the desktop webviews on the PC
  - [x] I6c Node extension-host sidecar started by the Rust core, workspace trust (untrusted folder = no host), no gateway keys ([design](features/node-extensions-design.md), accepted 2026-11-22: VSCodium server downloaded on first need. Built: I6c-1 core (#20), I6c-2 editor wiring (install, trust, download and start, connection), I6c-3 real VSCodium 1.140.06880 pins, checked on the PC; PC click-through fixed 3 bugs and found that the server must match the editor's VS Code version (editor 1.145, newest VSCodium 1.143), now checked before connecting. I6c-4 (Pheonix chose to build our own server): scripts/build_node_host.py, Linux x64 1.145.2 server built and working with the real editor in the cloud. Published as pre-release node-host-1.145.2 on the IDE repo (made public 2026-11-22) and pinned for Linux x64 and Windows x64. Done 2026-11-22: Code Spell Checker from Open VSX runs in the Windows desktop app. macOS and Linux arm64 wait for a builder)
  - [x] I6d Optional Electron compatibility host for extensions with Electron-built native modules (download on first need, pinned and hashed, off by default). Done 2026-11-22: the same pinned server runs under Electron 43.8.3 in Node mode; a test extension with an Electron-only native module runs with it in the cloud ([design section 11](features/node-extensions-design.md#11-electron-compatibility-host-i6d)). Still to do: the Windows desktop app on the PC
- [x] **I7** TandemForge local engine: the IDE pairs with a running TandemForge and uses its local models (owner's choice 2026-11-21; this branch; [features/tandemforge-engine.md](features/tandemforge-engine.md)). Still to do: both desktop apps side by side on the PC with a real model

## Side tracks

- [x] **X** VSCodium / VS Code extension ([page](features/vscode-extension.md)). Its own TypeScript gateway client and agent tools and the IDE's Rust ones run the same test cases (`core/tests/vectors/gateway_client.json`, `agent_tools.json`, `missions.json`), so they behave the same. Phases: X1 chat and gateway picker, X2 local agent tools with diff review, X3 missions
  - [x] X1 chat view and gateway picker (2026-11-22; real VS Code 1.95.0 and 1.147.0)
  - [x] X2 local agent tools with diff review (2026-11-22; shared agent-tool test cases with the IDE; real VS Code 1.95.0 and 1.147.0)
  - [x] X3 missions (2026-11-22; shared mission test cases with the IDE; real VS Code 1.95.0 and 1.147.0)
- [ ] **Accounts** Email sign-in on gateways with universal accounts (ADR-030 phase A3, built with the P2P-AI accounts work)

## Also shipped

- App icons with the IDE badge (#2); installers, bundle settings and licence notices (#6); workspace access through a
  folder handle against link swaps (#8); third-party notices for the sandbox crates (#13); README and user guide (#14);
  npm audit gate (#15).

## Not planned now

- Signed installers and releases: paused until everything is implemented and tested (Pheonix, 2026-11-21).
