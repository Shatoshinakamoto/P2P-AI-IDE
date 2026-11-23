# Notes for 2026-11-23_032814Z_security-scan_ffb62c3

## 2026-11-23 03:28 UTC

App shell: 0 vulnerabilities, 6 allowed warnings (unmaintained or unsound crates in the Tauri and Linux desktop stack: chrono, glib, proc-macro-error, zbus_names, zvariant, zvariant_derive). They were already in the lockfile before this change; this change adds no dependency.
