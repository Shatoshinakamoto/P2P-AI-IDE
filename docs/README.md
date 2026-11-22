# P2P-AI IDE documentation

The IDE's plan and its decisions (ADR-029) live in the P2P-AI repository: `docs/restructure/agentic-ide-plan.md`.

| Document | Description |
|---|---|
| [ROADMAP.md](ROADMAP.md) | Roadmap and checklist: where each phase (I0 to I7, side track X) stands, with the PR that shipped it |
| [user-guide.md](user-guide.md) | **The user guide, a screenshot tour**: every view of the IDE, what it does and how to use it |
| [commands/docs-screenshots.md](commands/docs-screenshots.md) | `app/tests/docs-shots.mjs`: retakes the screenshots in `docs/images/` |
| [CHANGELOG.md](CHANGELOG.md) | Every addition, change, fix and removal, newest first |
| [SECURITY-LOG.md](SECURITY-LOG.md) | Security fixes, hardening and accepted risks |
| [features/editor-shell.md](features/editor-shell.md) | Phase I1: open a folder, explorer, Monaco tabs, save with conflict check, search; threat model and controls |
| [features/gateways-and-chat.md](features/gateways-and-chat.md) | Phase I2a: saved gateways (local and manual), keys in the OS keychain, streamed chat, chat history; threat model and controls |
| [features/public-gateways.md](features/public-gateways.md) | Phase I2b: public gateway list from signed directory adverts, verified like the node; threat model and controls |
| [features/agent-tools.md](features/agent-tools.md) | Phase I3: chat tools on the open folder (list, read, search, propose edits) and the diff review; threat model and controls |
| [features/missions.md](features/missions.md) | Phase I4: send the folder to a gateway project, Autopilot and Auto Improve missions, approvals, proposals, review of returned work; threat model and controls |
| [features/source-control.md](features/source-control.md) | Phase I5a: the Source Control view (status, diff, stage, commit) through the installed git, with repository programs and hooks off; threat model and controls |
| [features/run-and-test.md](features/run-and-test.md) | Phase I5b: the Run and test view and the chat's runs with per-run approval, on the gateway sandbox; threat model and controls |
| [features/local-sandbox.md](features/local-sandbox.md) | Phase I5c: the local OS sandbox on this computer (Linux: namespaces, Landlock, seccomp), ported from TandemForge; threat model and controls |
| [features/container-runner.md](features/container-runner.md) | Phase I5d: the local container runner (Docker or Podman) with pinned Red Hat UBI images; threat model and controls |
| [features/extensions.md](features/extensions.md) | Phases I6a and I6b: the Extensions view, Open VSX search and verified install, themes, grammars and snippets, web extensions in a sandboxed worker; threat model and controls |
| [features/node-extensions-design.md](features/node-extensions-design.md) | Phase I6c: design for Node extensions (our own VS Code server build downloaded on first need, folder trust, local sidecar); Linux x64 build pinned, Windows x64 next, then a real Node extension in the desktop app |
| [files/jschardet-stub.md](files/jschardet-stub.md) | `app/stubs/jschardet`: the stand-in that keeps an unused LGPL library out of the installers |
| [files/app-data.md](files/app-data.md) | The IDE's own data files: `gateways.json`, `directories.json`, `links.json` and chat history, every key |
| [files/discovery-vectors.md](files/discovery-vectors.md) | The copy of the P2P-AI protocol's shared advert vectors used by the core tests, with provenance |
| [commands/app-scripts.md](commands/app-scripts.md) | The npm scripts and cargo commands used to build and test the app |
| [commands/live-gateway-e2e.md](commands/live-gateway-e2e.md) | The live check of missions and runs against a real P2P-AI gateway, node and sandbox (development only) |
| [commands/local-toolchains.md](commands/local-toolchains.md) | The live check of real toolchains (cargo, go, npm, make, unittest) inside the local sandbox (development only) |
| [commands/local-containers.md](commands/local-containers.md) | The live check of the container runner with a real engine, and every engine command and flag the IDE uses (development only) |
| [commands/build-node-host.md](commands/build-node-host.md) | `scripts/build_node_host.py`: builds the Node extension host server from the editor's VS Code version |
| [commands/history-test.md](commands/history-test.md) | The test history recorder `scripts/history_test.py` (copied from the P2P-AI repository) |
| [files/third-party-notices.md](files/third-party-notices.md) | `app/src-tauri/THIRD-PARTY-NOTICES.txt`: the licence texts shipped in the installers, how it is generated and checked |
| [features/tandemforge-engine.md](features/tandemforge-engine.md) | Phase I7: use TandemForge on this computer as a model engine (pairing code, loopback only); threat model and controls |
| [features/installers.md](features/installers.md) | Windows and Linux installers and signed releases, built on the PC with the P2P-AI release script |
| [files/app-icons.md](files/app-icons.md) | The app icons: P2P-AI logo with the IDE badge, how they are built |
| [configuration/tauri-conf.md](configuration/tauri-conf.md) | `tauri.conf.json` (window, CSP) and `capabilities/default.json`, every key |
| [commands/npm-audit-gate.md](commands/npm-audit-gate.md) | The npm audit gate `scripts/npm_audit_gate.py`: shipped deps always strict, dated dev-only exceptions |
| [files/npm-audit-exceptions.md](files/npm-audit-exceptions.md) | `app/npm-audit-exceptions.json`: the dev-only advisory exceptions and why each is there |
| [../HistoryTest/README.md](../HistoryTest/README.md) | The record of every test run |
