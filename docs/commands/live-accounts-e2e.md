# `scripts/live_accounts_e2e.mjs`: live check of "Sign in with a P2P-AI account"

Development test only; not part of the app. Added 2026-11-23 for phase A3 ([features/account-sign-in.md](../features/account-sign-in.md)).

## Purpose

The core's unit tests use a fake account service. This script runs the real P2P-AI account service (`p2p-accounts`)
and a real Go gateway with universal accounts on, both on 127.0.0.1, plus a tiny fake model node. It then runs the
IDE core's example `core/examples/live_accounts.rs`, which calls the same core functions as the app, while headless
Chromium plays the person on the account service's sign-in page.

## Synopsis

```
node scripts/live_accounts_e2e.mjs --bin-dir DIR [--chromium PATH]
```

No root needed. Linux and macOS (Windows untested).

| Option | Argument | Default | What it does |
|---|---|---|---|
| `--bin-dir` | folder (required) | none | holds `p2p-accounts` and `gateway`, built in a P2P-AI checkout: `cd P2P-AI-Gateways/Go && go build -o DIR ./cmd/p2p-accounts ./cmd/gateway` |
| `--chromium` | path | Playwright's own browser | a Chromium binary to drive (for example `/opt/pw-browsers/chromium` in the cloud) |

Environment variables read: `PATH` and `HOME` (passed to the children), `NODE_PATH` (to find the machine's `playwright`
package). The children get an environment built from scratch.

## Security of the run

* SECURITY-WARNING: both services use plain http on 127.0.0.1 with `ACCOUNTS_ALLOW_INSECURE_HTTP=on`
  (INSECURE — DEV ONLY). Never use that switch on a reachable service.
* Every secret (account service admin key, gateway admin key, enrollment token, accounts key, passwords) is random per
  run, goes to children through 0600 files, their environment or stdin, never argv, and is never printed (CWE-214,
  CWE-532). The work folder is 0700 and deleted at the end.
* Child processes get argument arrays, never a shell string (CWE-78).

## What it checks (19)

Account service up; gateway up with accounts on; fake model node registered; operator makes the account with a one-time
password; the IDE opens the home's `/authorize` with `client_id=p2p-ai-ide`, PKCE S256 and a `127.0.0.1` return; the
browser lands on the IDE's "Signed in" page; the refresh token is in the secret store and not in `accounts.json`; the
email is shown; no gateway token before the one-time password is replaced; a short new password is refused before it
is sent; the password is changed from the IDE; a gateway token is issued; the second call uses the cache; the gateway
lists models with it; a streamed chat is answered; a token for another gateway id is refused by this gateway; sign-out
revokes and forgets; no token after sign-out; the example exits cleanly.

## Output and exit codes

`PASS name` / `FAIL name (detail)` lines, then `SUMMARY: n passed, n failed`. Exit 0 = all passed, 1 = a check failed
or setup failed.

## Examples

```
# Cloud (Linux), binaries built from P2P-AI main:
NODE_PATH=$(npm root -g) node scripts/live_accounts_e2e.mjs --bin-dir /tmp/a3bin --chromium /opt/pw-browsers/chromium

# With Playwright installed in the app folder:
NODE_PATH=app/node_modules node scripts/live_accounts_e2e.mjs --bin-dir ../P2P-AI/bin
```
