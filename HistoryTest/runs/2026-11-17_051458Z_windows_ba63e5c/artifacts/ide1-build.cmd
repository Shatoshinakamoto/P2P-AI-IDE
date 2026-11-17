@echo off
rem ide1-build.cmd -- Windows build and tests of the P2P-AI-IDE phase I1 shell at ba63e5ca (PR #1).
rem Everything happens under D:\p2p-ai-ci\ide1; nothing else on the PC is touched. No admin rights needed.
rem Steps: hash the source, extract, npm ci (no install scripts), type check + unit tests + webview build,
rem Rust core fmt/clippy/tests, Tauri shell fmt/clippy, release binary. Log: D:\p2p-ai-ci\logs\ide1.txt
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide1-src-ba63e5ca.tar.gz
if not exist logs mkdir logs
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
if exist ide1 rmdir /s /q ide1
mkdir ide1
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
echo ide1-done
) > logs\ide1.txt 2>&1
