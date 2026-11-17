@echo off
rem ide1b-build.cmd -- rebuild the P2P-AI-IDE release exe at e30cb3e (blank-window fix) on the Windows PC.
rem Only D:\p2p-ai-ci\ide1 is touched; the cargo target folder from the first build is reused. No admin rights.
rem Steps: hash the source, extract over the old tree, npm ci (no install scripts), npm test, shell clippy, release.
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide1-src-e30cb3e.tar.gz
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
tar -xzf %SRC% -C ide1 && echo extracted
set CARGO_TARGET_DIR=D:\p2p-ai-ci\ide1\target
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
cd /d D:\p2p-ai-ci\ide1\app
call npm ci --ignore-scripts --no-fund --no-audit
echo npm-ci-exit !ERRORLEVEL!
call npm test
echo npm-test-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide1\app\src-tauri
cargo clippy --all-targets --locked -- -D warnings
echo shell-clippy-exit !ERRORLEVEL!
cd /d D:\p2p-ai-ci\ide1\app
call npm run release
echo release-exit !ERRORLEVEL!
dir D:\p2p-ai-ci\ide1\target\release\p2p-ide.exe
echo ide1b-done
) > D:\p2p-ai-ci\logs\ide1b.txt 2>&1
