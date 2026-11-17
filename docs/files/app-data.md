# The IDE's own data files (app data folder)

Since phase I2 the IDE keeps a little state of its own. It lives in the per-user app data folder that Tauri picks for
the identifier `org.p2p-ai.ide`:

| OS | Folder |
|---|---|
| Windows | `%APPDATA%\org.p2p-ai.ide\` |
| macOS | `~/Library/Application Support/org.p2p-ai.ide/` |
| Linux | `~/.local/share/org.p2p-ai.ide/` |

Only the Rust core reads or writes these files (`core/src/store.rs`); the webview cannot name a path. Every write is
atomic (temp file, then rename). On Unix files are 0600 and folders 0700. Nothing here is a secret: **keys are in the
OS keychain**, under service `org.p2p-ai.ide.gateway`, account = the gateway's id.

## gateways.json

Saved gateways and the active one. Read and re-checked on every command; a damaged or hand-edited file with a bad
entry is refused as a whole ("a saved file is damaged or too large").

```json
{
  "gateways": [
    { "id": "3f2a9c0d1e4b5a67", "name": "This computer", "url": "https://127.0.0.1:8443", "kind": "local",
      "ca_pem": "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n" },
    { "id": "a1b2c3d4e5f60718", "name": "Team gateway", "url": "https://gateway.example.org", "kind": "manual" }
  ],
  "active": "3f2a9c0d1e4b5a67"
}
```

| Key | Type | Allowed values | Security impact |
|---|---|---|---|
| `gateways[].id` | string | 16 lowercase hex | names the keychain entry; allow-listed (CWE-20) |
| `gateways[].name` | string | 1-64 chars, one line | display only |
| `gateways[].url` | string | `https://host[:port]`, or `http://127.0.0.1|localhost|[::1]:port` (dev) | where the key is sent; no path, user info, query (CWE-918, NIST SC-8) |
| `gateways[].kind` | string | `local`, `manual` | display only |
| `gateways[].ca_pem` | string, optional | PEM certificates only, max 64 KiB, no private key | when present, the only trusted root for that gateway (CWE-295) |
| `active` | string, optional | an id in the list | which gateway chat uses |

Limits: 32 gateways, file 4 MiB. Change it through the Gateways view, not by hand.

## chats/&lt;scope&gt;/&lt;id&gt;.json

One file per chat. `scope` is the first 16 hex characters of SHA-256 of the open folder's path (so each folder has its
own history and the path is not in the folder name), or `none` when no folder is open.

| Key | Type | Allowed values |
|---|---|---|
| `id` | string | 16 lowercase hex; must match the file name |
| `title` | string | up to 120 chars, one line (from the first message) |
| `gateway` | string | display name of the gateway used last (not its URL or key) |
| `model` | string | up to 200 chars |
| `created`, `updated` | number | seconds since 1970 |
| `messages[]` | `{role, content, at}` | role `user`, `assistant` or `error` (a local error line, never sent to a model); up to 400 messages |

Limits: 500 chats per folder, 4 MiB per chat. Delete chats in the History list (two clicks); deleting the folder
removes all history.

**Privacy:** chats can contain code you pasted and the model's answers. They stay on this computer; the IDE does not
upload them anywhere except the messages you send to the gateway you chose.
