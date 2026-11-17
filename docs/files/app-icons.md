# App icons (`app/src-tauri/icons/`)

The IDE's window, taskbar and installer icons: the official P2P-AI logo (blue-to-purple brain with network nodes,
chosen by Pheonix on 2026-11-17) with a purple `</>` badge in the lower-right corner, so the IDE is recognisable next
to the other P2P-AI apps (desktop app: blue monitor, website: globe, node: server stack).

| File | Format | Read by |
|---|---|---|
| `32x32.png`, `128x128.png`, `128x128@2x.png` (256), `icon.png` (512) | PNG, transparent background | Tauri bundler (Linux, window icon) |
| `icon.ico` | ICO with 16, 24, 32, 48, 64, 128, 256 px PNG entries | `tauri-build` (Windows resource, required) |
| `icon.icns` | ICNS 16 to 1024 px | macOS bundle |

All entries added 2026-11-17 at Pheonix's request ("For our logo across all applications is this", "add something to
it to make it different for each one"). They replace the earlier placeholder icons.

**Do not edit these by hand.** They are built from the one source logo by `branding/make_icons.py` in the P2P-AI
repository, which keeps the build reproducible and reviewable (NIST SSDF PS.3):
```bash
# in a P2P-AI checkout next to this one
python3 -I branding/make_icons.py --ide-root ../P2P-AI-IDE
python3 -I branding/verify_icons.py . ../P2P-AI-IDE
```
Design, security notes and the other apps' badges: `docs/features/branding.md` in the P2P-AI repository. Changes to
the logo are approved by the project owner.

Risks: a hand-edited icon drifts from the source (the generator's `--check` catches it); a missing `icon.ico` stops
the Windows build.
