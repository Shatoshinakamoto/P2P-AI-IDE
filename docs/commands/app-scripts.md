# App scripts (`app/package.json`)

Run from `app/`. No script needs administrator rights.

| Script | Synopsis | What it does | Exit codes |
|---|---|---|---|
| `dev` | `npm run dev` | Vite dev server on `127.0.0.1:5174` (`--strictPort`; loopback only) | Vite's |
| `build` | `npm run build` | `tsc --noEmit` type check, then `vite build` into `dist/` (no source maps) | 0 ok, non-zero on a type or build error |
| `check` | `npm run check` | type check only | as `tsc` |
| `test` | `npm test` | unit tests with `node --test tests/*.test.ts` | 0 when all pass |
| `ui-smoke` | `npm run ui-smoke` | the UI smoke test (`tests/ui-smoke.mjs`) on the built `dist/` in headless Chromium; needs Playwright installed; env `SHOT=<file>` sets the screenshot path (default `app/ui-smoke.png`, git-ignored) | 0 when every check passes |
| `release` | `npm run release` | `tauri build --no-bundle`: release binary, no installer | Tauri's |
| `tauri` | `npm run tauri -- <args>` | the Tauri CLI, e.g. `npm run tauri dev` | Tauri's |

Install with `npm ci --ignore-scripts`: dependency install scripts never run (CWE-829). Third-party CLIs used:
[Vite](https://vite.dev/guide/cli), [Tauri CLI](https://v2.tauri.app/reference/cli/), `tsc`, `node --test`.

Rust commands (from `core/` or `app/src-tauri/`): `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo test --locked`, `cargo audit --file Cargo.lock`. `--locked` refuses to change the lockfile, so builds use the
reviewed versions.
