@echo off
rem ide5b-build.cmd -- Windows build and tests of P2P-AI-IDE phase I5b at 391278c (PR #9).
rem Everything happens under D:\p2p-ai-ci\ide5b; nothing else on the PC is touched. No admin rights needed.
rem Steps: hash the source, extract, Rust core fmt/clippy/tests, npm ci (no install scripts), webview tests and
rem build, npm audit, Tauri shell fmt/clippy, release binary. Log: D:\p2p-ai-ci\logs\ide5b.txt
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide5b-src-391278c.tar.gz
if not exist logs mkdir logs
if not exist ide5b mkdir ide5b
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
tar -xzf %SRC% -C ide5b && echo extracted
set CARGO_TARGET_DIR=D:\p2p-ai-ci\ide1\target
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
cd /d D:\p2p-ai-ci\ide5b\core
cargo fmt -- --check
echo core-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo core-clippy-exit !ERRORLEVEL!
cargo test --locked
echo core-test-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide5b\app
call npm ci --ignore-scripts --no-fund --no-audit
echo npm-ci-exit !ERRORLEVEL!
call npm test
echo npm-test-exit !ERRORLEVEL!
call npm run build
echo build-exit !ERRORLEVEL!
call npm audit
echo npm-audit-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide5b\app\src-tauri
cargo fmt -- --check
echo shell-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo shell-clippy-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide5b\app
call npm run release
echo release-exit !ERRORLEVEL!
dir D:\p2p-ai-ci\ide1\target\release\p2p-ide.exe
echo ide5b-done
) > logs\ide5b.txt 2>&1
