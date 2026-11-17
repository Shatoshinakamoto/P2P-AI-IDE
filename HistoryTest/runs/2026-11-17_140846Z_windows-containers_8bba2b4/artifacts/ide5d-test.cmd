@echo off
rem ide5d-test.cmd -- Windows checks of the phase I5d container runner at 8bba2b4 with Docker Desktop.
rem Everything happens under D:\p2p-ai-ci\ide5d; no admin rights needed. --prepare pulls the three pinned Red Hat
rem UBI images by digest and builds the local Rust image (network needed only for that step). Runs then use
rem --pull=never --network=none. Steps: hash, extract, core fmt/clippy/tests, then the real container example.
rem Log: D:\p2p-ai-ci\logs\ide5d.txt
setlocal EnableDelayedExpansion
cd /d D:\p2p-ai-ci
set SRC=ide5d-src-8bba2b4.tar.gz
if not exist logs mkdir logs
if exist ide5d rmdir /s /q ide5d
mkdir ide5d
(
echo == date %DATE% %TIME%
certutil -hashfile %SRC% SHA256
tar -xzf %SRC% -C ide5d && echo extracted
set CARGO_TARGET_DIR=D:\p2p-ai-ci\ide1\target
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
cd /d D:\p2p-ai-ci\ide5d\core
cargo fmt -- --check
echo core-fmt-exit !ERRORLEVEL!
cargo clippy --all-targets --locked -- -D warnings
echo core-clippy-exit !ERRORLEVEL!
cargo test --locked
echo core-test-exit !ERRORLEVEL!
echo == containers %TIME%
cargo run --locked --quiet --example local_containers -- --prepare
echo containers-exit !ERRORLEVEL!
echo == end %TIME%
echo ide5d-done
) > D:\p2p-ai-ci\logs\ide5d.txt 2>&1
