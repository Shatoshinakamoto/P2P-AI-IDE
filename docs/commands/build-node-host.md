# `scripts/build_node_host.py` - build the Node extension host server

**Purpose.** Builds the server that runs Node extensions (VS Code's "remote extension host", REH) from VS Code's MIT source, at exactly the VS Code version the editor library is built on. Node extensions only work when the two match (see [features/node-extensions-design.md](../features/node-extensions-design.md) section 7). Pheonix chose on 2026-11-22 to build our own server instead of waiting for a VSCodium release of the same version.

The server contains native modules, so **each OS builds its own** (Linux on Linux, Windows on Windows, macOS on a Mac). The CPU can be chosen with `--arch` where VS Code's build supports cross-building.

**Synopsis.**

```
python3 scripts/build_node_host.py --tag TAG --commit SHA --out DIR [--work DIR] [--arch x64|arm64]
```

**Privileges.** An ordinary user. No root, no admin. It needs the network for the VS Code source (github.com), npm packages (registry.npmjs.org), the server's Node (nodejs.org) and ripgrep (github.com).

## Options

| Option | Type, default | What it does | Security |
|---|---|---|---|
| `--tag` | `N.N.N`, required | VS Code release tag to build. Use the editor library's version: `vscode/src/vs/platform/product/common/product.js` in `@codingame/monaco-vscode-api` (1.145.2 for 39.0.5). | Format allow-listed (CWE-20). |
| `--commit` | 40 lowercase hex, required | The tag's commit. The build stops when the clone is at any other commit, and again when the built server's `product.json` names another. | Source pin (SSDF PS.2, CWE-494). |
| `--out` | folder, required | Where the archive, its `.sha256` and the build record go. Created if missing. | |
| `--work` | folder, default `<out>/work` | Build folder (the VS Code clone, ~2 GB with `node_modules`, and the built server). Reused when present: the clone is checked again, `npm ci` is skipped if `node_modules` exists. Delete it for a clean build. | A clone with changes other than the two this script makes stops the build. |
| `--arch` | `x64` or `arm64`, default this machine's | Target CPU. | Allow-listed. |

## What it does

1. Clones `https://github.com/microsoft/vscode.git` at `--tag` (depth 1) and checks the commit.
2. Checks Node against the tag's `.nvmrc` (same major, not older), VS Code's own rule.
3. Changes two files in the clone:
   * `product.json`: `quality` = `stable` (the editor announces `stable`), `builtInExtensions` = `[]` (leaves out the marketplace debugger extensions js-debug, js-debug-companion and js-profile-table: the IDE has no debugger UI for them).
   * `build/gulpfile.reh.ts`: removes the one step that prepares the Copilot extension's ripgrep shim. The build stops if that line changed upstream.
4. `npm ci` (lockfile with integrity hashes), then `npm run gulp vscode-reh-<platform>-<arch>-min`.
5. Removes the GitHub Copilot extension and the Copilot runtime packages (`node_modules/@github/copilot*`, about 170 MB) from the built server. The IDE has its own agent and doesn't ship a client for another AI service (least functionality, NIST CM-7).
6. Writes `p2pide-node-host-<platform>-<arch>-<tag>.tar.gz` (one top folder; sorted entries, owner 0, the commit's date as mtime, gzip mtime 0), its `.sha256`, and `<name>.json` (tag, commit, Node, OS, time, size, SHA-256).

The archive is what the app downloads and unpacks after checking the SHA-256 pinned in `core/src/node_host.rs`.

## Build machine needs

| OS | Needs |
|---|---|
| Linux | Node of the tag's `.nvmrc` (1.145.2: 24.22.1), git, python3, gcc/g++, make, `libkrb5-dev`, `libx11-dev`, `libxkbfile-dev`, `libsecret-1-dev`, `pkg-config` |
| Windows | Node as above, git, Python, Visual Studio (2022 or later) with the C++ workload **and "MSVC Spectre-mitigated libs"** (VS Code's native modules are built with Spectre mitigation; without them `kerberos` fails with MSB8040), and the Windows SDK (installed with the C++ workload). The script adds the newest SDK's `x64` folder from `C:\Program Files (x86)\Windows Kits\10\bin` to the end of `PATH` when `signtool.exe` isn't on it: VS Code's packaging step calls `signtool verify` and `signtool remove /s` to strip invalidated signatures from native modules (it never signs) |
| macOS | Node as above, git, Xcode command line tools |

## Environment variables

Read: `PATH` (to find git, node, npm, and on Windows signtool). Set for the build: `NODE_OPTIONS=--max-old-space-size=8192`, `ELECTRON_SKIP_BINARY_DOWNLOAD=1`, `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`, `VSCODE_ARCH`, `npm_config_arch`.

## Exit codes and output

0 when the archive and record are written; otherwise a message saying which step failed (clone, commit check, Node version, `npm ci`, gulp, product check). The build record is also printed as JSON.

## Examples

```
# Linux x64, the editor's current version
python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 --out dist-node-host

# Windows x64 on the PC, with a portable Node of the right version first on PATH
set PATH=D:\p2p-ai-ci\node-host-build\node-v24.22.1-win-x64;%PATH%
python scripts\build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 --out D:\p2p-ai-ci\node-host-build\out --work D:\p2p-ai-ci\node-host-build\work

# Try a build with the real editor before pinning it: start it through the core, then run the UI smoke test against it
cargo run --manifest-path core/Cargo.toml --example node_host_serve -- --tgz dist-node-host/p2pide-node-host-linux-x64-1.145.2.tar.gz --sha256 <hex> --data /tmp/nh-data --folder /tmp/nh-ws > /tmp/nh.json
(cd app && npm run build && NODE_HOST_REAL=/tmp/nh.json NODE_HOST_REAL_DATA=/tmp/nh-data npm run ui-smoke)
```

`node_host_serve` prints the server's connection token for the test on this machine only; don't run it where stdout is logged or shared.

## Limitations

* Builds are as repeatable as VS Code's own build: the archive's metadata is fixed, but bundler output can still differ between machines. The pinned SHA-256 is of the published file, whichever machine built it.
* The cloud environment's proxy refuses `api.github.com`; there, fill `@vscode/ripgrep`'s cache (`/tmp/vscode-ripgrep-cache-<version>/`) from the same file on github.com before the build. The script doesn't do this.
* GitHub Actions is billing-locked (2026-11), so there is no macOS or Linux arm64 builder yet.

## Test evidence

* Linux x64 build: [2026-11-22_053759Z_node-host-build_4096b2e](../../HistoryTest/runs/2026-11-22_053759Z_node-host-build_4096b2e/RUN.md)
* That build with the real editor: [2026-11-22_053720Z_ui-node-host-real_832a0a7](../../HistoryTest/runs/2026-11-22_053720Z_ui-node-host-real_832a0a7/RUN.md)
