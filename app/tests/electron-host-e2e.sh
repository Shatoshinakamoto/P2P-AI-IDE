#!/usr/bin/env bash
# electron-host-e2e.sh -- phase I6d end-to-end check of the Electron compatibility host (development only).
#
# Purpose: installs a Node extension host server archive and (in "electron" mode) an Electron zip through the core's
# own checks (core/examples/node_host_serve: SHA-256, safe unpack, scrubbed start), stages the abi-probe test
# extension (native module built for Electron's ABI only), runs the editor's UI smoke test against that real server
# and reports whether the extension activated. Expected: "activated" with electron, "FAILED" with node (the
# control). Exit code: 0 when the outcome is the expected one for the mode, 1 otherwise.
#
# Usage: electron-host-e2e.sh <node|electron>
# Environment (all required): SERVER_TGZ (our server .tar.gz, with SERVER_TGZ.sha256 beside it), ELECTRON_ZIP and
# ELECTRON_SHA256 (electron mode), PROBE_DIR (the built app/tests/fixtures/abi-probe).
# Security: temp folders from mktemp, removed at the end; the connection token stays in a 0600 file and is never
# printed (CWE-532); nothing is downloaded by this script; the server only listens on 127.0.0.1.
set -euo pipefail
mode="${1:?node or electron}"
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
base="$(mktemp -d)"; trap 'rm -rf "$base"' EXIT
data="$base/data"; ws="$base/ws"; ext="$data/extensions/p2p-ai-test.abi-probe-0.0.1"
mkdir -p "$ext" "$ws"; printf '# notes\n' > "$ws/notes.md"
cp -r "$PROBE_DIR/package.json" "$PROBE_DIR/extension.js" "$PROBE_DIR/native" "$ext/"
zeros="$(printf '0%.0s' $(seq 64))"
cat > "$data/extensions/installed.json" <<J
[{"id":"p2p-ai-test.abi-probe","namespace":"p2p-ai-test","name":"abi-probe","version":"0.0.1","display_name":"ABI probe","description":"test","kind":"node","enabled":true,"installed_at":0,"sha256":"$zeros","verified_publisher":false,"folder":"p2p-ai-test.abi-probe-0.0.1","network":false}]
J
extra=()
if [ "$mode" = electron ]; then extra=(--electron-zip "$ELECTRON_ZIP" --electron-sha256 "$ELECTRON_SHA256"); fi
(cd "$repo/core" && cargo build -q --example node_host_serve)
fifo="$base/stdin"; mkfifo "$fifo"
( umask 077; exec "$repo/core/target/debug/examples/node_host_serve" --tgz "$SERVER_TGZ" --sha256 "$(cut -d' ' -f1 "$SERVER_TGZ.sha256")" \
    --data "$data" --folder "$ws" "${extra[@]}" < "$fifo" > "$base/serve.json" 2>"$base/serve.err" ) &
exec 9>"$fifo"
for _ in $(seq 120); do [ -s "$base/serve.json" ] && break; sleep 0.5; done
echo "host program: $(ps -eo args | grep -F -- "--connection-token-file $data" | grep -v grep | head -1 | awk '{print $1}' | sed "s#$base#<tmp>#")"
cd "$repo/app"
set +e
NODE_HOST_REAL="$base/serve.json" NODE_HOST_REAL_DATA="$data" SHOT_NODE="$base/shot.png" npm run ui-smoke 2>&1 | grep -E "^(PASS|FAIL)|passed"
rc=${PIPESTATUS[0]}
set -e
log="$(cat "$data"/node-host-state/user/logs/*/exthost*/remoteexthost.log 2>/dev/null || true)"
outcome=none
if grep -q "Activating extension p2p-ai-test.abi-probe failed" <<<"$log"; then
  outcome=failed; echo "abi-probe: FAILED to activate"
  grep -m1 -o "NODE_MODULE_VERSION [0-9]*. This version of Node.js requires NODE_MODULE_VERSION [0-9]*" <<<"$log" || true
elif grep -q "_doActivateExtension p2p-ai-test.abi-probe" <<<"$log"; then outcome=activated; echo "abi-probe: activated with no error"
else echo "abi-probe: not started"; fi
echo "ui-smoke exit $rc"
exec 9>&-; wait || true
# Exit 0 only for the expected outcome: activated with a clean UI smoke run under Electron; the ABI failure under
# plain Node (the control).
if [ "$mode" = electron ] && [ "$outcome" = activated ] && [ "$rc" = 0 ]; then echo "e2e: pass (electron)"; exit 0; fi
if [ "$mode" = node ] && [ "$outcome" = failed ]; then echo "e2e: pass (expected failure on plain Node)"; exit 0; fi
echo "e2e: fail"; exit 1
