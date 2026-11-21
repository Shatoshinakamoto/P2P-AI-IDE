#!/usr/bin/env bash
# Static checks and supply-chain scans for phase I6a (run from the IDE repository root).
set -euo pipefail
echo "== tsc"; (cd app && npx tsc --noEmit && echo ok)
echo "== cargo fmt core"; (cd core && cargo fmt --check && echo ok)
echo "== clippy core"; (cd core && cargo clippy -q --all-targets -- -D warnings && echo ok)
echo "== fmt shell"; (cd app/src-tauri && cargo fmt --check && echo ok)
echo "== clippy shell"; (cd app/src-tauri && cargo clippy -q --all-targets -- -D warnings && echo ok)
echo "== cargo audit core"; (cd core && cargo audit)
echo "== cargo audit shell"; (cd app/src-tauri && cargo audit)
echo "== npm audit gate"; python3 scripts/npm_audit_gate.py app
echo "== licence notices --check"; python3 -I "$NOTICES" --app-dir app --check
