# Security log

Every security patch, hardening change, CVE remediation and accepted risk, newest first.

## 2026-11-17 — Accepted risk: two RustSec warnings in Tauri's Linux GTK stack
- **Type:** accepted risk
- **CVE / CWE / advisory:** RUSTSEC-2024-0370 (proc-macro-error unmaintained), RUSTSEC-2024-0429 (glib
  `VariantStrIter` unsoundness). No CVE named by cargo audit.
- **Severity:** warnings, not vulnerabilities (cargo audit 0.22.1, 2026-11-17). CVSS/EPSS not applicable; KEV no.
- **Affected:** `app/src-tauri/Cargo.lock`, pulled in by tauri 2.12.5 through wry and gtk-rs 0.18 on Linux only.
- **What changed:** nothing; the IDE does not call `glib::VariantStrIter`, and proc-macro-error runs only at build
  time. The P2P-AI desktop app has the same pair.
- **Why it is acceptable:** no reachable code path; fixed upstream when Tauri moves off gtk-rs 0.18. CI runs cargo
  audit on every change, so a real vulnerability fails the build.
- **Verification:** `HistoryTest/runs/2026-11-17_045933Z_security-scan_fe528ba`.

## 2026-11-17 — Monaco's vendored DOMPurify replaced by the patched release
- **Type:** CVE remediation (dependency)
- **CVE / CWE / advisory:** GitHub advisories GHSA-p98j-92pf-mc4p, GHSA-6688-9rhm-gjv2, GHSA-cg2g-xh64-v4q7,
  GHSA-mrv2-3pqr-j5jr, GHSA-c6gr-qvv4-5v3f (DOMPurify <= 3.4.16), as reported by `npm audit` on 2026-11-17; CWE-79.
  No CVE IDs were listed by npm audit, so none are named here.
- **Severity:** npm audit rated the dompurify entry critical. CVSS and EPSS not looked up; KEV not checked.
  Reachability in this app is low: Monaco calls DOMPurify with `RETURN_DOM_FRAGMENT` and none of `IN_PLACE`,
  `SAFE_FOR_TEMPLATES` or `PARSER_MEDIA_TYPE` (checked in `monaco-editor/esm/vs/base/browser/domSanitize.js`), which
  the advisories need. Fixed anyway rather than accepted.
- **Affected:** monaco-editor 0.57.0, which pins `dompurify` 3.4.15 and also ships its own copy at
  `esm/vs/base/browser/dompurify/dompurify.js`.
- **What changed:** `app/package.json` adds `dompurify` 3.4.18 and an npm `overrides` entry, and `app/vite.config.ts`
  redirects Monaco's import of its vendored copy to that package, so the bundle contains only 3.4.18.
- **Why it is secure now:** the sanitizer Monaco runs on hover and markdown content is the release with the fixes
  (NIST SI-2 flaw remediation, SSDF RV.2).
- **Verification:** `npm audit` 0 vulnerabilities and `npm ls dompurify` = 3.4.18
  (`HistoryTest/runs/2026-11-17_045808Z_security-scan_fe528ba`); the built bundle has `version=3.4.18` and no
  `3.4.15` (`HistoryTest/runs/2026-11-17_045746Z_webview_fe528ba`); the UI smoke test passes on that bundle
  (`HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`). Remove the redirect once a Monaco release ships 3.4.17+.

## 2026-11-17 — Accepted risk: `style-src 'unsafe-inline'` for Monaco
- **Type:** accepted risk
- **CVE / CWE / advisory:** none; CWE-79 (defence in depth reduced for styles only)
- **Severity:** low. Scripts stay `script-src 'self'` with no inline scripts or eval, so injected markup cannot run code;
  inline styles allow at most visual tricks or CSS-based data probing within the page.
- **Affected:** `app/src-tauri/tauri.conf.json` CSP.
- **What changed:** Monaco creates `<style>` elements and style attributes at run time, which a strict `style-src 'self'`
  blocks (the editor does not render).
- **Why it is acceptable:** all file text is rendered as text by React and Monaco, not as HTML; there is no remote
  content and `connect-src` allows only Tauri's IPC, so CSS cannot send data out (NIST SC-18 partial).
- **Verification:** UI smoke test under the shipped CSP reports no CSP violations
  (`HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`). Revisit with Monaco's nonce support in phase I6.

## 2026-11-17 — Phase I1 controls (hardening)
- **Type:** hardening (new feature with its controls)
- **CVE / CWE / advisory:** none. CWE-22, CWE-59, CWE-73, CWE-367, CWE-400, CWE-1333, CWE-209, CWE-117, CWE-284.
- **Severity:** not scored (no vulnerability fixed).
- **Affected:** `core/src/workspace.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/capabilities/default.json`.
- **What changed:** path allow-list and canonical confinement, link refusal on save, versioned atomic saves, caps,
  plain-text search, folder chosen only through the OS picker in Rust, no plugin permissions for the webview, fixed
  error messages and path-free audit lines. Details: [features/editor-shell.md](features/editor-shell.md) section 3.
- **Why it is secure now:** NIST AC-6, AU-3, SI-10; ASVS V12.3.1, V14.4.
- **Verification:** `HistoryTest/runs/2026-11-17_045740Z_core_fe528ba` (10 unit tests with malicious paths, links,
  sizes and conflicts), `HistoryTest/runs/2026-11-17_045806Z_ui-smoke_fe528ba`.
