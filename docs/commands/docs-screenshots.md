# Documentation screenshots: `app/tests/docs-shots.mjs`

**Added 2026-11-17.** Retakes the ten pictures in `docs/images/` used by [README.md](../../README.md) and the
[user guide](../user-guide.md), so they always match the current interface.

## Synopsis

```
cd app
npm ci --ignore-scripts && npm run build
node tests/docs-shots.mjs [output-folder]     # default: ../docs/images
```

**Privileges:** a normal user; nothing is installed and nothing needs administrator rights.
**Needs:** Playwright with Chromium (as for `npm run ui-smoke`; not a project dependency).

| Argument | Type | Default | Effect |
|---|---|---|---|
| `output-folder` | path | `docs/images/` | Where the PNG files go; created if missing; files of the same name are overwritten |

No flags and no environment variables. Exit code 0 when every picture was written; a page or button that is not found
stops the script with a Playwright error and a non-zero exit code.

## What it does

1. Serves `app/dist/` on `127.0.0.1` with the CSP from `src-tauri/tauri.conf.json`.
2. Loads the fake Tauri bridge of `tests/ui-smoke.mjs` (taken as source text, so the two never drift apart) and, on
   top of it, a small sample project ("shop-api"), a friendly agent answer and a proposed change.
3. Walks the IDE like a user: welcome, open folder and editor, search, source control, add and check a gateway and a
   public directory, ask the agent, review the change, start an Autopilot mission, run the tests, and the run approval
   card. Window size 1400 x 860.

Pictures: `welcome`, `editor`, `search`, `source-control`, `gateways`, `agent-chat`, `review-change`, `missions`,
`run-and-test`, `run-approval` (`.png`).

## Security

- **Sample data only**: made-up files, gateway (`gateway.example.org`) and placeholder key
  (`p2p_example_placeholder_key`, not a secret). ui-smoke's hostile strings (kept there on purpose to prove they render
  as text, CWE-79) are swapped for plain text in the pictures only.
- **Loopback only** and **not shipped**: a development tool; it is not part of the app or the installers.

## Troubleshooting

| Problem | Fix |
|---|---|
| `ui-smoke.mjs changed shape` | The `fake` function in `tests/ui-smoke.mjs` was renamed; update the markers in `docs-shots.mjs` |
| `Executable doesn't exist` | `npx playwright install chromium` |
| A label is not found | The interface wording changed; update the label in the script |
