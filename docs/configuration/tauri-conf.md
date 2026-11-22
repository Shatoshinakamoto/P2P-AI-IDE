# `app/src-tauri/tauri.conf.json` and `capabilities/default.json`

Both are JSON, which has no comments, so every key is documented here (CLAUDE.md section 4).

## `tauri.conf.json`

Location: `app/src-tauri/tauri.conf.json`. Read by the Tauri CLI and compiled into the binary at build time; a change
needs a rebuild. Schema: <https://schema.tauri.app/config/2>.

| Key | Value | Security impact |
|---|---|---|
| `productName`, `version`, `identifier` | `P2P-AI IDE`, `0.1.0`, `org.p2p-ai.ide` | identifier decides the app data folder; keep it unique |
| `build.frontendDist` | `../dist` | only the built webview files are packed |
| `build.devUrl` | `http://127.0.0.1:5174` | dev only; loopback, never the LAN |
| `build.beforeDevCommand`, `beforeBuildCommand` | `npm run dev`, `npm run build` | fixed commands |
| `app.withGlobalTauri` | `false` | no `window.__TAURI__` global for page scripts |
| `app.windows[0]` | label `main`, 1400x900, min 960x600 | the label ties the window to the capability below |
| `app.security.csp` | see below | the main XSS control (ASVS V14.4) |
| `app.security.freezePrototype` | `false` | `true` would freeze `Object.prototype` against prototype pollution (CWE-1321), but Monaco assigns `toString` on its own objects and stops with a TypeError, leaving a blank window. Off is an accepted risk (SECURITY-LOG 2026-11-17); the UI smoke test applies the freeze whenever this is `true`, so turning it back on is tested |
| `bundle.publisher`, `copyright`, `category`, `shortDescription`, `longDescription`, `homepage`, `license` | `P2P-AI`, Apache-2.0 copyright line, `DeveloperTool`, ... | **Added 2026-11-17.** Shown by the installers and package managers |
| `bundle.licenseFile` | `../../LICENSE` | The licence page of the NSIS and MSI installers |
| `bundle.resources` | `THIRD-PARTY-NOTICES.txt`, `../../LICENSE`, `../../NOTICE` | Licence texts installed next to the app ([files/third-party-notices.md](../files/third-party-notices.md)) |
| `bundle.windows.digestAlgorithm` | `sha256` | Authenticode digest when signing is on (NIST SP 800-131A) |
| `bundle.windows.certificateThumbprint`, `timestampUrl` | `null` | No Authenticode for now (Pheonix's choice, 2026-11-17); the release script sets them per build when a certificate exists |
| `bundle.windows.webviewInstallMode` | `downloadBootstrapper`, silent | Microsoft's signed WebView2 bootstrapper, only where WebView2 is missing |
| `bundle.windows.nsis.installMode` | `currentUser` | Per-user install, no administrator prompt (NIST AC-6) |
| `bundle.*` | icons, all targets | Installers are built and signed by `scripts/release_app.py --app ide` in the P2P-AI repository ([docs](https://github.com/Shatoshinakamoto/P2P-AI/blob/main/docs/features/desktop-installers.md)); SHA256SUMS is cosign-signed, Authenticode later. `bundle.icon` lists `icons/32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.icns`, `icon.ico`: the P2P-AI logo with the purple `</>` IDE badge, built by `branding/make_icons.py --ide-root` in the P2P-AI repository (see [files/app-icons.md](../files/app-icons.md)) |

CSP directives:

| Directive | Value | Why |
|---|---|---|
| `default-src` | `'self'` | nothing remote by default |
| `script-src` | `'self' 'wasm-unsafe-eval' 'unsafe-eval' 'sha256-2EBQn+AFduI8Wb0riLwbeshEGP4dJlHornyvuFAgYnA='` | only the app's own bundled scripts. `'wasm-unsafe-eval'` (phase I6a) lets the bundled oniguruma WebAssembly module compile (TextMate colours). Phase I6b: the hash allows the one inline script of the web worker extension host iframe (monaco-vscode-api 39.0.5; a new version needs the new hash), and `'unsafe-eval'` lets that iframe's worker load web extension code. **The IDE page itself does not get `'unsafe-eval'`**: its `<meta>` CSP (`app/vite.config.ts`, the same string without it) applies on top, and the stricter of the two wins |
| `style-src` | `'self' 'unsafe-inline'` | Monaco creates `<style>` elements at run time. Accepted risk, SECURITY-LOG 2026-11-17 |
| `img-src`, `font-src` | `'self' data: blob:` | Monaco's icon font and small inline images; `blob:` (phase I6a) for icons and fonts an installed icon theme contributes, which the page turns into `blob:` URLs from bytes the Rust core read |
| `worker-src` | `'self' blob:` | Monaco's language workers (bundled files; `blob:` for Monaco's fallback) |
| `connect-src` | `'self' data: blob: ipc: http://ipc.localhost ws://127.0.0.1:* http://127.0.0.1:*` | Tauri's own IPC, the app's bundled files, and `data:`/`blob:` URLs made in the page (phase I6a: the editor services load built-in and installed extension files this way). Phase I6c: `ws://` and `http://` on 127.0.0.1 let the page reach the local Node extension host (its port is chosen by the OS, hence `:*`). Web extensions don't get these: `app/vite.config.ts` removes local sources from the extension host iframe's own CSP, and browsers apply both policies. No remote host: Open VSX and gateways are reached only by the Rust core |
| `object-src`, `base-uri`, `form-action` | `'none'` | no plugins, base tag tricks or form posts |
| `frame-src` | `'self'` (phase I6b) | only the app's own extension host iframe page |
| `frame-ancestors` | `'self'` (phase I6b; was `'none'`) | the app may frame its own pages (the extension host iframe); no other site can frame the IDE. Header only: the meta CSP leaves it out, as browsers ignore it there |

Secure values: never add a remote host to `script-src` or `connect-src`; network access belongs in the Rust core.
Never remove the `iframeCsp` plugin from `vite.config.ts` while `connect-src` names 127.0.0.1: it keeps web extensions
away from local services, and the build fails if the iframe page still names a local source.
Never remove the page CSP plugin from `vite.config.ts` while `'unsafe-eval'` is here; the build refuses a page CSP
that contains it. See [features/extensions.md](../features/extensions.md#web-extensions-i6b).

## `capabilities/default.json`

Location: `app/src-tauri/capabilities/default.json`. Read by `tauri-build` at build time.

| Key | Value | Security impact |
|---|---|---|
| `identifier` | `default` | name only |
| `windows` | `["main"]` | applies to the main window |
| `permissions` | `["core:default"]` | Tauri's basic window and event permissions only. No plugin permission: the webview cannot call the dialog plugin or any file, shell or HTTP API. The app's own commands are always callable and do their own checks |

Adding a permission widens what a compromised page could do (NIST AC-6). Any change needs a review and a SECURITY-LOG
entry.

## Example

The files in the repository are the complete, annotated example; they hold no secrets.
