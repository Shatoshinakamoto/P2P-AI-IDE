# Sign in with a P2P-AI account (universal accounts, phase A3)

* **Status:** built 2026-11-23 (ADR-030 phase A3 in the P2P-AI repository, `docs/features/universal-accounts-design.md`).
  Requested by Pheonix (roadmap item **Accounts**).
* **Code:** `core/src/accounts.rs` (sign-in, token cache), `core/src/gateways.rs` (`account` on a gateway,
  `set_account`), `app/src-tauri/src/account.rs` (commands, system browser), `app/src-tauri/src/net.rs` (`TokenKeychain`,
  token in place of a key), `app/src/components/AccountsPanel.tsx`, `app/src/components/NetworkView.tsx`.
* **Server side:** the P2P-AI account service `p2p-accounts` (the "home") and Go gateways with
  `GATEWAY_ACCOUNTS_ACCEPT=on` (P2P-AI `docs/features/account-service.md`, `docs/features/gateway-universal-accounts.md`).

## 1. Overview

Before this, every gateway needed its own API key pasted into the IDE. A gateway whose operator turned on universal
accounts now accepts a short token from the person's account service instead. In the IDE the person:

1. signs in once at the account service, in their normal browser (the IDE never sees the password);
2. links a saved gateway to that account with the gateway's **account id** (its `GATEWAY_ID`, given by the operator).

From then on chat, the agent, missions and runs on that gateway use a fresh 5-minute token that the IDE asks the
account service for. The gateway turns the account into an ordinary user key of its own, so its rate limits, caps and
usage work as for any key.

## 2. How it works

```mermaid
sequenceDiagram
  participant W as IDE webview
  participant C as IDE core (Rust)
  participant B as System browser
  participant H as Account service (home)
  participant G as Gateway
  W->>C: account_sign_in(home)
  C->>C: listen on 127.0.0.1:<random>/cb; PKCE verifier + S256 challenge; state
  C->>B: open home/authorize?client_id=p2p-ai-ide&redirect_uri=http://127.0.0.1:<port>/cb&...
  B->>H: sign-in page (email + password, 2-step code or passkey)
  H-->>B: 303 to http://127.0.0.1:<port>/cb?code&state&iss
  B->>C: GET /cb (state and iss checked) -> "Signed in" page
  C->>H: POST /token code + verifier, resource=home
  H-->>C: refresh token (-> OS keychain) + account token (-> /v1/me for the email)
  W->>C: chat on a gateway linked to the account
  C->>H: POST /token refresh, resource=<GATEWAY_ID>, scope=chat projects:read projects:write missions
  H-->>C: new refresh token (-> keychain first) + access token (memory, ~4 min)
  C->>G: Authorization: Bearer <access token>
```

* **Remembered homes** are in `accounts.json` in the app data folder (address, email, no secret;
  [files/app-data.md](../files/app-data.md#accountsjson)). The refresh token is in the OS keychain under service
  `org.p2p-ai.ide.account`, account = the home's id (first 16 hex of SHA-256 of its address).
* **A gateway's link** (`account: {home, audience}`) is saved on its entry in `gateways.json`. When it is set, the core
  uses an account token for that gateway instead of its stored key; "Use key instead" removes the link (the key, if any,
  stays in the keychain). TandemForge on this computer never takes a link (it uses its pairing code).
* **Token cache:** one access token per home and gateway in memory, used until a minute before it runs out. Every use
  of a refresh token is serialised: the home rotates the token on each use and treats a reused old one as stolen.
* **One-time passwords:** an account the operator made starts with a one-time password and gets no gateway token until
  the person picks their own. The IDE shows "choose your own password" and a **Change password** form.

## 3. How it is secure

| Threat (STRIDE) | Control | Reference |
|---|---|---|
| Code stolen on the way back (Spoofing) | PKCE S256, 256-bit verifier from the OS CSPRNG; the code is useless without it | RFC 7636, CWE-330 |
| Another site or local process sends a code to our listener (CSRF, injection) | 256-bit `state` compared in constant time; only `GET /cb`; repeated parameters refused; up to 32 requests and 5 minutes | RFC 6749 10.12, CWE-352, CWE-235, CWE-400 |
| Code from a different home (mix-up) | `iss` must equal the home the sign-in started at | RFC 9207 |
| Password seen by the app | Sign-in happens in the system browser, never an embedded view | RFC 8252 8.12 |
| Return address re-pointed | Redirect to the IP literal `127.0.0.1` with a random port (the home also refuses anything else for `p2p-ai-ide`) | RFC 8252 7.3, 8.3, CWE-601 |
| Tokens leak (Information disclosure) | Refresh token in the OS keychain only; access tokens in memory only; neither in files, logs, errors, the webview, the Node extension host's environment or command lines | CWE-522, CWE-532, CWE-214, NIST IA-5 |
| Reused refresh token kills the session | Refreshes serialised under one lock; the rotated token is stored before the access token is used | RFC 9700 4.14.2, CWE-362 |
| Token used beyond its purpose (Elevation) | Gateway tokens ask only for `chat projects:read projects:write missions`; `account` only for the home itself; the gateway allows only listed routes per scope | NIST AC-6, CWE-269 |
| Token for gateway A sent to gateway B | One token per gateway id (audience); the answer's audience must match; gateways refuse other audiences | RFC 9068, CWE-294 |
| Plain-text tokens on the wire | Homes are `https` with the Mozilla roots; plain http only on loopback, shown as **INSECURE — DEV ONLY**; no proxy, no redirects | NIST SC-8, CWE-319, CWE-295 |
| Browser started through a shell (Tampering) | The link is one argument to `xdg-open`, `open` or `rundll32 url.dll,FileProtocolHandler`; only a link starting with the checked home + `/authorize?` and holding URL-safe characters | CWE-78 |
| Hand-edited files | `accounts.json` and `gateways.json` are re-checked on load (address normalised, id = hash of address, audience grammar) | CWE-20 |
| Errors reveal details | Fixed messages; the home's error text is never shown | CWE-209 |
| Repudiation | Audit lines (stderr, fixed words): signed in, sign-in failed, signed out, password changed or failed, gateway linked or unlinked | NIST AU-2, AU-3, CWE-117 |

These are implemented controls, not a compliance assessment.

## 4. How to use it

1. Ask the gateway's operator for two things: the **account service address** (for example
   `https://accounts.example.org`) and the gateway's **account id** (for example `gw-berlin-01`).
2. Open **Gateways** in the IDE. Under **P2P-AI accounts**, type the address and press **Sign in**. Your browser opens
   the sign-in page. Sign in there; the tab then says "Signed in" and you can close it.
3. If the IDE says "choose your own password", open **Change password**, type the one-time password and a new one
   (15 characters or more), and press **Change**. Your other signed-in devices are signed out.
4. On the gateway's card press **Use account**, pick the address, type the account id and press **Link**. The card now
   shows "account you@example.org".
5. Use chat, the agent, missions and runs as usual. **Sign out** ends this app's session at the account service.

For a test on one computer, run the account service with `ACCOUNTS_ALLOW_INSECURE_HTTP=on` on `http://127.0.0.1:<port>`
(INSECURE — DEV ONLY); see [commands/live-accounts-e2e.md](../commands/live-accounts-e2e.md).

### VS Code and VSCodium extension

The extension (`vscode-extension/src/accounts.ts`, 2026-11-23) is a TypeScript port of `core/src/accounts.rs` with the
same rules, messages and client id (`p2p-ai-ide`):

* **P2P-AI: Sign In with a P2P-AI Account** asks for the address, opens the home's sign-in page with
  `vscode.env.openExternal` (the system browser, never a webview) and waits in a cancellable notification. If no browser
  opens, it offers to copy the sign-in link (the link holds no secret; the PKCE verifier stays in the extension).
* **P2P-AI: Change Account Password** (password boxes), **P2P-AI: Sign Out of a P2P-AI Account**.
* **P2P-AI: Use a P2P-AI Account for a Gateway** links a saved gateway (account service + the gateway's account id);
  **P2P-AI: Use the Key for a Gateway** goes back. The key stays saved.
* Refresh tokens are in VS Code's SecretStorage (`p2pAi.accountRefresh.<home id>`, the OS keychain where VS Code has
  one); remembered homes (no secret) are in the extension's state file under `p2pAi.accounts`; access tokens only in
  memory. The gateway client asks for the current token before every request (`renew`), so a long agent turn or the
  Missions view's timer never sends an expired token.
* Audit lines (fixed words) go to the **P2P-AI** output channel.

## 5. Dependencies

No new crates or npm packages (the extension uses Node's built-in `http`, `https` and `crypto`). The sign-in uses crates the core already had: ureq 3.5.2 (rustls), serde_json 1.0.154,
sha2 0.10.9, base64 0.22.1, getrandom 0.2.17; the shell uses keyring 4.2.0. `cargo audit` was run on 2026-11-23 (see
the security-scan run in section 8).

## 6. Limitations and known risks

* The account id is typed by hand: gateways do not yet announce their account home and id. A wrong id gives "the
  gateway refused the key" (the home issues a token for any id unless it limits audiences to its hub).
* A home with a private CA is not supported yet (public roots only, or loopback http for development).
* Passkeys and two-step codes work because they happen on the home's own page; managing passkeys and sessions from the
  IDE is not built yet (use the home's API).
* Ending one session at the home does not revoke access tokens already issued from it; they run out within 5 minutes.
* The VS Code extension uses VS Code's SecretStorage; where VS Code has no OS keychain (some Linux desktops) it keeps
  secrets in its own weaker store. It may also route requests through VS Code's configured proxy (TLS stays end to end).
* On Linux the browser opens through `xdg-open`; without a desktop session the sign-in cannot start.

## 7. Troubleshooting

| Message | Cause and fix |
|---|---|
| "the account service address must be https://..." | Use the https address; plain http only on 127.0.0.1 for development |
| "could not open the system browser" | No default browser (or no `xdg-open`); set one, then try again |
| "the sign-in took longer than 5 minutes" | Press Sign in again and finish within 5 minutes |
| "the sign-in was refused or cancelled in the browser" | The page was closed or the home refused; sign in again |
| "this account must choose its own password first" | Use Change password with the one-time password |
| "the sign-in has ended; sign in again" | The session was ended (password change elsewhere, sign-out, operator, or a reused token); sign in again |
| "the account service does not issue tokens for this gateway" | The home only serves gateways its hub lists; check the account id with the operator |
| "the gateway refused the key" on a linked gateway | The gateway does not accept this home, or the account id is wrong |

## 8. Test evidence

* Core unit tests (PKCE, link building, listener, `state`/`iss` checks, rotation, cache, sign-out, edited files) and
  the gateway link test: HistoryTest `core` run of this change.
* UI smoke (sign in, one-time password, link, unlink, sign out): HistoryTest `ui` run of this change.
* Real end to end, 19 checks: `scripts/live_accounts_e2e.mjs` with the P2P-AI account service and Go gateway built from
  P2P-AI main and a real headless Chromium: HistoryTest `live-accounts` run of this change.
* VS Code extension, on the Windows PC: type check ([run](../../HistoryTest/runs/2026-11-23_035759Z_vscode-ext-check_9243fa4/RUN.md),
  0 errors) and unit tests including 9 account tests ([run](../../HistoryTest/runs/2026-11-23_035800Z_vscode-ext-unit_9243fa4/RUN.md), 44 of 44).

The run folders are listed in the [CHANGELOG](../CHANGELOG.md) entry of 2026-11-23.
