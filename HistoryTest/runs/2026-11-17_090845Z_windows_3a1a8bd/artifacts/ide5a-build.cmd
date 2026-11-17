@echo off
rem ide5a-build.cmd -- Windows build and tests of P2P-AI-IDE phase I5a at 3a1a8bd (PR #7).
rem Everything happens under D:\p2p-ai-ci\ide1; nothing else on the PC is touched. No admin rights needed.
rem Steps: hash the source, extract, Rust core fmt/clippy/tests, npm ci (no install scripts), webview tests and
rem build, npm audit, Tauri shell fmt/clippy, release binary. Log: D:\p2p-ai-ci\logs\ide5a.txt
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide5a-src-3a1a8bd.tar.gz
if not exist logs mkdir logs
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
tar -xzf %SRC% -C ide1 && echo extracted
set CARGO_TARGET_DIR=D:\p2p-ai-ci\ide1\target
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
cd /d D:\p2p-ai-ci\ide1\core
cargo fmt -- --check
echo core-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo core-clippy-exit !ERRORLEVEL!
cargo test --locked
echo core-test-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide1\app
call npm ci --ignore-scripts --no-fund --no-audit
echo npm-ci-exit !ERRORLEVEL!
call npm test
echo npm-test-exit !ERRORLEVEL!
call npm run build
echo build-exit !ERRORLEVEL!
call npm audit
echo npm-audit-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide1\app\src-tauri
cargo fmt -- --check
echo shell-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo shell-clippy-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide1\app
call npm run release
echo release-exit !ERRORLEVEL!
dir D:\p2p-ai-ci\ide1\target\release\p2p-ide.exe
echo ide5a-done
) > logs\ide5a.txt 2>&1
