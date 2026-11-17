# `app/src-tauri/THIRD-PARTY-NOTICES.txt`

**Added 2026-11-17** (thread "Ship signed installers and releases", requested by Pheonix).

## What it is

The copyright and licence texts of every third-party package that can ship inside the IDE: the Rust crates Cargo
resolves for Windows, Linux and macOS (x86_64 and aarch64) and the npm packages of the web UI that are not dev-only
(467 packages on 2026-11-17). Each distinct licence text appears once, followed by the packages it covers; MPL-2.0
packages are listed with their source URL. `tauri.conf.json` bundles it into every installer (`bundle.resources`),
next to `LICENSE` and `NOTICE`.

## Who writes and reads it

* Written by `scripts/third_party_notices.py` in the P2P-AI repository; **never edit it by hand**.
* Checked by the same script with `--check` before every installer build (`scripts/release_app.py build --app ide`),
  which also fails on any licence outside the allow-list.
* Read by people who install the IDE (it is installed next to the executable).

## How to update it

After any dependency change (`Cargo.lock` or `package-lock.json`):

```bash
(cd app && npm ci && cargo fetch --locked --manifest-path src-tauri/Cargo.toml)
python3 -I ../P2P-AI/scripts/third_party_notices.py --app-dir app
```

Commit the result in the same pull request as the dependency change.

## Policy and risks

The allow-list (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, CDLA-Permissive-2.0, MPL-2.0 and similar; no GPL, AGPL,
SSPL or "no licence") and the review results are in the P2P-AI repository's `docs/files/licence-review.md`. A stale
file ships wrong attributions, which the `--check` gate prevents; widening the allow-list without review could ship
a copyleft licence the project cannot meet.
