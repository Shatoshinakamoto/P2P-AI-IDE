@echo off
rem ide5a2-core.cmd -- re-runs the P2P-AI-IDE Rust core checks at 71c6f12 (PR #7) on Windows after the test fix.
rem Works only under D:\p2p-ai-ci\ide5a2 (own target folder, so the running release build is not disturbed).
rem No admin rights, no network beyond the cargo registry cache. Log: D:\p2p-ai-ci\logs\ide5a2.txt
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide5a2-src-71c6f12.tar.gz
if not exist ide5a2 mkdir ide5a2
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
tar -xzf %SRC% -C ide5a2 && echo extracted
set CARGO_TARGET_DIR=D:\p2p-ai-ci\ide5a2\target
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
cd /d D:\p2p-ai-ci\ide5a2\core
git --version
cargo fmt -- --check
echo core-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo core-clippy-exit !ERRORLEVEL!
cargo test --locked
echo core-test-exit !ERRORLEVEL!
echo ide5a2-done
) > D:\p2p-ai-ci\logs\ide5a2.txt 2>&1
