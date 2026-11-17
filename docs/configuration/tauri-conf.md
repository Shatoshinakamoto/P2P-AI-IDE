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
| `bundle.*` | icons, all targets | installers come later (no signing yet). `bundle.icon` lists `icons/32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.icns`, `icon.ico`: the P2P-AI logo with the purple `</>` IDE badge, built by `branding/make_icons.py --ide-root` in the P2P-AI repository (see [files/app-icons.md](../files/app-icons.md)) |

CSP directives:

| Directive | Value | Why |
|---|---|---|
| `default-src` | `'self'` | nothing remote by default |
| `script-src` | `'self'` | only the app's own bundled scripts; no inline scripts, no eval |
| `style-src` | `'self' 'unsafe-inline'` | Monaco creates `<style>` elements at run time. Accepted risk, SECURITY-LOG 2026-11-17 |
| `img-src`, `font-src` | `'self' data:` | Monaco's icon font and small inline images |
| `worker-src` | `'self' blob:` | Monaco's language workers (bundled files; `blob:` for Monaco's fallback) |
| `connect-src` | `ipc: http://ipc.localhost` | only Tauri's own IPC; no network from the page (gateways in phase I2 go through Rust) |
| `object-src`, `base-uri`, `frame-ancestors`, `form-action` | `'none'` | no plugins, base tag tricks, framing or form posts |

Secure values: never add a remote host to `script-src` or `connect-src`; network access belongs in the Rust core.

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
