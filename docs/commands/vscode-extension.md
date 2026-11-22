# `vscode-extension/` npm scripts

**Purpose.** Build, test and package the P2P-AI extension for VS Code and VSCodium
([features/vscode-extension.md](../features/vscode-extension.md)).

**Synopsis.** Run from `vscode-extension/` after `npm ci`:

```
npm run check | build | test | test:integration | package
```

**Privileges.** An ordinary user. `test:integration` needs a display (on Linux without one: `xvfb-run -a`).

| Script | What it does | Network | Security notes |
|---|---|---|---|
| `check` | `tsc -p . --noEmit`: type-checks `src/` | none | |
| `build` | `tsc -p .`: compiles `src/` to `out/` (CommonJS, `.ts` imports rewritten to `.js`) | none | |
| `test` | `node --test tests/*.test.ts`: shared gateway-client cases, the client against a fake gateway on 127.0.0.1 (key only in `Authorization`, no redirects, caps, Stop, fixed errors), the webview message check, a pinned-CA TLS test (skipped without `openssl`), and the agent tools (shared cases from `core/tests/vectors/agent_tools.json`, links refused, caps, lost-update check, pending edits, turn budget) in temp folders | loopback only | Test certificates and folders are made in the temp folder and deleted; on Windows the file-link check is skipped when creating a link needs admin rights |
| `test:integration` | `npm run build`, then `node tests/integration/run.mjs [version]`: downloads VS Code (default `stable`; pass a version such as `1.95.0`) into `.vscode-test/`, starts it with `--disable-extensions`, a fresh temp profile, a throwaway project folder (trusted with `--disable-workspace-trust`) and this extension, and runs `tests/integration/suite.cjs` in its extension host against a fake gateway, including an agent turn, the diff review and Accept | update.code.visualstudio.com, then loopback | Sets `P2P_AI_EXT_TEST=1`; the extension's test hooks exist only when VS Code runs it in test mode |
| `package` | `npm run build`, then `vsce package --no-dependencies --allow-missing-repository --out p2p-ai.vsix` | none | `.vscodeignore` keeps sources, tests and `node_modules` out; the package has no run-time dependencies |

`vsce` flags used: `--no-dependencies` (nothing from `node_modules` is packed; the extension has no run-time
dependencies), `--allow-missing-repository` (skips vsce's repository-field prompt), `--out` (file name). Full list:
<https://github.com/microsoft/vscode-vsce>.

**Environment variables.** Read by the extension: `P2P_AI_EXT_TEST` (test mode only, see above). Set by
`run.mjs`: `P2P_AI_EXT_TEST=1` and `P2P_AI_TEST_PROJECT` (the throwaway project folder, read by the suite) for the
VS Code it starts.

**Webview smoke (not an npm script).** `node tests/ui/view-smoke.cjs <out.png>` draws `media/chat.js` and
`chat.css` in Chromium with a stub of the webview API and a sample state (tool steps, two pending changes), clicks
Accept and checks the view posts only `{type: "accept", rel}`. It needs Playwright with Chromium, which is not a
project dependency (it is preinstalled in the cloud test containers). It prints `view-smoke: pass` or
`view-smoke: fail` and exits non-zero on failure. No network.

**Exit codes.** 0 on success; non-zero when a type check, test or package step fails (`run.mjs` prints
`integration: pass` or `integration: fail`).

## Examples

```sh
cd vscode-extension && npm ci
npm test                                   # unit tests, about 1 s
xvfb-run -a npm run test:integration       # newest stable VS Code
npm run build && xvfb-run -a node tests/integration/run.mjs 1.95.0   # oldest supported VS Code
npm run package && code --install-extension p2p-ai.vsix
```
