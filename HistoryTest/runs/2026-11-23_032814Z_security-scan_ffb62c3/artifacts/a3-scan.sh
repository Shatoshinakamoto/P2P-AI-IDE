#!/usr/bin/env bash
# A3 security scan: RustSec on both lockfiles, npm audit on the app, gitleaks on the changed tree when available.
set -euo pipefail
fail=0
echo "== cargo audit core"; (cd core && cargo audit --file Cargo.lock) || fail=$((fail+1))
echo "== cargo audit app shell"; (cd app/src-tauri && cargo audit --file Cargo.lock) || fail=$((fail+1))
echo "== npm audit app (high+)"; (cd app && npm audit --audit-level=high) || fail=$((fail+1))
if command -v gitleaks >/dev/null; then echo "== gitleaks"; gitleaks detect --no-git --source core/src --source app/src -v || fail=$((fail+1)); else echo "== gitleaks not installed (skipped)"; fi
echo "== grep for token prefixes in the changed source"; if grep -rn "p2prt_[A-Za-z0-9]\{8,\}" core/src app/src app/src-tauri/src; then fail=$((fail+1)); else echo "none"; fi
echo "failed checks: $fail"; exit $fail
