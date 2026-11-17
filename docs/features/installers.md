# Installers and signed releases

**Added 2026-11-17** (thread "Ship signed installers and releases", requested by Pheonix).

## Overview

The IDE ships as a Windows NSIS setup `.exe` (per-user, no admin), a Windows `.msi` (final versions), and Linux
`.deb` and `.rpm` packages. Each release has `SHA256SUMS`, cosign-signed with the P2P-AI project key, a CycloneDX
SBOM, `BUILD-INFO-<platform>.json` and the licence texts. Releases are uploaded as **drafts** to this repository's
GitHub Releases; the owner publishes them.

Pheonix's choice (2026-11-17): no Authenticode certificate yet, so Windows shows an "unknown publisher" warning;
the cosign-signed checksums are the integrity check. macOS waits for a Mac and an Apple Developer ID.

## How it works and how it is secure

The build, signing and upload are done by `scripts/release_app.py --app ide --repo <this checkout>` in the P2P-AI
repository, so both desktop apps share one audited release path and one key. The full description, threat model,
controls (clean tagged tree, licence gate, pinned non-root Linux builder, signed checksums, draft-only upload) and
troubleshooting are in P2P-AI's
[docs/features/desktop-installers.md](https://github.com/Shatoshinakamoto/P2P-AI/blob/main/docs/features/desktop-installers.md).

## How to use it

On the Windows PC, with both repositories checked out side by side:

```powershell
cd D:\path\to\P2P-AI-IDE; git tag -s v0.1.0 -m "P2P-AI IDE v0.1.0"; git push origin v0.1.0
cd D:\path\to\P2P-AI
python -I scripts\release_app.py build  --app ide --repo ..\P2P-AI-IDE --version v0.1.0 --platform windows
python -I scripts\release_app.py build  --app ide --repo ..\P2P-AI-IDE --version v0.1.0 --platform linux
python -I scripts\release_app.py sign   --app ide --version v0.1.0      # COSIGN_PASSWORD in the environment
python -I scripts\release_app.py upload --app ide --version v0.1.0
```

## Dependencies

No new dependency in this repository. Tools: Tauri CLI 2.12.4 (package-lock), Rust (MSVC on Windows), Node 22,
Docker for the Linux build, gh for the upload.

## Limitations

No Authenticode, no macOS, x64 only. `tauri.conf.json`'s `version` must equal the tag.

## Test evidence

* [Licence gate and Linux bundle build in the cloud](../../HistoryTest/README.md) (runs of 2026-11-17, area
  `installers`). Windows installers and the upload: pending the first PC run.
