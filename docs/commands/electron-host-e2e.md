# `app/tests/electron-host-e2e.sh`

**Purpose.** Phase I6d development check: runs the real Node extension host server, through the core's own
install and start code, with or without the Electron compatibility host. It stages the `abi-probe` test extension,
whose native module is built for Electron 43.8.3 only. Then it connects the real editor (the UI smoke test with
`NODE_HOST_REAL`) and reports whether the extension activated. Not part of the app.
([features/node-extensions-design.md](../features/node-extensions-design.md#11-electron-compatibility-host-i6d))

**Synopsis.**

```
SERVER_TGZ=<server.tar.gz> ELECTRON_ZIP=<electron.zip> ELECTRON_SHA256=<64 hex> PROBE_DIR=<fixture> \
  app/tests/electron-host-e2e.sh <node|electron>
```

**Privileges.** An ordinary user. Needs a built app (`npm run build` in `app/`), Playwright's Chromium, Rust, and on
Linux the desktop libraries Electron links to.

| Argument | Values | What it does |
|---|---|---|
| `node` | | Starts the server with its own Node 24 (the control): the probe must fail with module version 148 vs 137 |
| `electron` | | Installs Electron from `ELECTRON_ZIP` through `electron_host::ensure_installed` (hash, safe unzip) and starts the server with it in Node mode: the probe must activate |

| Variable | Required | Meaning |
|---|---|---|
| `SERVER_TGZ` | yes | Our server archive from `scripts/build_node_host.py`, with `<file>.sha256` beside it |
| `ELECTRON_ZIP` | electron mode | The Electron 43.8.3 zip for this computer |
| `ELECTRON_SHA256` | electron mode | Its SHA-256 (use the pin from `core/src/electron_host.rs`) |
| `PROBE_DIR` | yes | `app/tests/fixtures/abi-probe` after building its native module ([fixture README](../../app/tests/fixtures/abi-probe/README.md)) |

**Security.** Nothing is downloaded by the script. Temp folders come from `mktemp` and are removed at the end. The
server listens on 127.0.0.1 only, and the connection token stays in a 0600 file. It is never printed (CWE-532).

**Output and exit codes.** It prints the host program (temp path shortened to `<tmp>`), the UI smoke lines, an
`abi-probe:` line and `e2e: pass` or `e2e: fail`. It exits 0 when the outcome is the expected one for the mode, and
1 otherwise.

**Examples.**

```sh
cd app && npm run build && cd ..
(cd app/tests/fixtures/abi-probe/native && npx -y node-gyp@11.5.0 rebuild --target=43.8.3 \
   --dist-url=https://electronjs.org/headers --runtime=electron)
export SERVER_TGZ=out/p2pide-node-host-linux-x64-1.145.2.tar.gz PROBE_DIR=$PWD/app/tests/fixtures/abi-probe
app/tests/electron-host-e2e.sh node                      # control: expected failure
ELECTRON_ZIP=electron-v43.8.3-linux-x64.zip \
ELECTRON_SHA256=73d7d6fe120c998db75834c23c8ce7ce8390607c198b2e167a28acd7937bef22 \
  app/tests/electron-host-e2e.sh electron               # compatibility host
```

`node-gyp` flags used: `--target` (Electron version), `--dist-url` (Electron's header server), `--runtime=electron`.
Full list: <https://github.com/nodejs/node-gyp>.
