# Security log

Every security patch, hardening change, CVE remediation and accepted risk, newest first.

## 2026-11-17 — Phase I2b hardening: signed public gateway list
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-347, CWE-345, CWE-694, CWE-918, CWE-350, CWE-601, CWE-295, CWE-294, CWE-330, CWE-79,
  CWE-400 (none: new feature)
- **Severity:** none (new feature built with these controls)
- **Affected:** `core/src/discovery.rs`, `core/src/client.rs` (public-only agent for kind `public`),
  `app/src-tauri/src/net.rs`, `app/src/components/PublicGateways.tsx`
- **What changed:** adverts are listed only after the same checks as the P2P-AI node (strict JSON, id bound to key,
  ed25519 signature before time, public https URLs, closed `client_access` values); a resolver drops private
  addresses after DNS for directories, probes and every request to a public-list gateway; probe nonces come from the
  OS CSPRNG and replies must be signed; the saved address comes from the verified cache, not the webview; advert text
  is shown as text, links not clickable; caps on every size and count.
- **Why it is secure now:** NIST SC-13 and SC-8, SI-10 (input validation), ASVS V5 (validation) and V12.6 (SSRF);
  the verifier passes the protocol's shared vectors, including all 25 refusal cases.
- **Verification:** HistoryTest runs 2026-11-17_063819Z_core_545bdf4 (41 tests),
  2026-11-17_063823Z_ui-smoke_545bdf4 (37/37), 2026-11-17_063832Z_security-scan_545bdf4 (0 vulnerabilities).

## 2026-11-17 — Phase I2a hardening: gateway keys, TLS and untrusted answers
- **Type:** hardening
- **CVE / CWE / advisory:** CWE-522, CWE-532, CWE-295, CWE-601, CWE-918, CWE-79, CWE-113, CWE-22, CWE-276, CWE-400
- **Severity:** none (new feature built with these controls)
- **Affected:** `core/src/{gateways,client,chats,store}.rs`, `app/src-tauri/src/net.rs`, `app/src/components/{NetworkView,ChatView}.tsx`
- **What changed:** keys only in the OS keychain and never returned to the webview; URL allow-list; TLS always on
  (pinned CA as the only root when set) except a badged loopback dev gateway; no redirects, no environment proxy;
  answers rendered as text only; state files owner-only and re-checked on load; caps on every size and time.
- **Why it is secure now:** NIST IA-5 and SC-8/SC-13, ASVS V9 (communications) and V6 (stored secrets), OWASP LLM01
  (answers treated as untrusted data). See docs/features/gateways-and-chat.md section 3.
- **Verification:** core tests (fake gateway: key in header only, no redirects, status mapping, stream caps, Stop),
  UI smoke test (key never shown, `<img onerror>` in an answer stays text). HistoryTest runs listed in the feature page.

## 2026-11-17 — Accepted risk: zbus 5 helper crates flagged unmaintained (Linux keychain)
- **Type:** accepted risk
- **CVE / CWE / advisory:** RUSTSEC-2026-0370 (zvariant), RUSTSEC-2026-0371 (zbus_names), RUSTSEC-2026-0372
  (zvariant_derive), all "unmaintained", informational; no vulnerability and no CVE.
- **Severity:** no CVSS (informational). KEV no.
- **Affected:** `app/src-tauri/Cargo.lock`, Linux builds only: keyring 4.2.0 -> zbus-secret-service-keyring-store
  1.0.1 -> zbus 5.19.0 -> zvariant 5.15.1, zbus_names 4.3.5.
- **What changed:** nothing. The crates were folded into zbus 6 (its `wire` module) by the same authors; the
  keyring store has not moved to zbus 6 yet. They only encode D-Bus messages to the local Secret Service.
- **Why it is acceptable now:** the D-Bus peer is the user's own session keyring service, not a network party; the
  data is the IDE's own key entries. Windows and macOS builds do not include these crates.
- **Follow-up:** `TODO(security)`: move to a keyring store on zbus 6 when released; cargo audit in CI keeps
  reporting them until then.
- **Verification:** `cargo audit --file app/src-tauri/Cargo.lock`: 0 vulnerabilities, 5 warnings (these 3 plus the
  2 already accepted). HistoryTest run listed in docs/features/gateways-and-chat.md.

## 2026-11-17 — Accepted risk: Object.prototype is not frozen (Monaco needs it writable)
- **Type:** accepted risk
- **CVE / CWE / advisory:** CWE-1321 (prototype pollution); no CVE.
- **Severity:** no score; this is a defence-in-depth setting, not a known vulnerability. KEV no.
- **Affected:** `app/src-tauri/tauri.conf.json` (`app.security.freezePrototype`), phase I1 release builds.
- **What changed:** `freezePrototype` went from `true` to `false`. With it on, Monaco's keyboard code
  (`KeyCodeUtils.toString = ...`) throws "Cannot assign to read only property 'toString'" while the bundle loads, so
  React never mounts and the window stays blank. Found on the Windows PC; the headless UI smoke test missed it
  because it did not apply the freeze.
- **Why it is acceptable now:** the webview runs only the app's own bundled scripts (`script-src 'self'`, no eval,
  no remote sources), so no outside script can pollute prototypes; file text and later model output are rendered as
  text, never run. The Rust commands validate every argument themselves (paths, sizes, versions) and do not trust
  object shapes from the webview. The UI smoke test now applies the freeze whenever the config turns it on, so a
  future Monaco that works with it can be re-enabled safely.
- **Verification:** UI smoke test with the freeze applied: fails with the same TypeError (reproduced); with the
  setting off: 16/16. Windows window check after the fix: see the HistoryTest runs listed in
  `docs/features/editor-shell.md`.

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
