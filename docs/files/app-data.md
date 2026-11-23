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
OS keychain**, under service `org.p2p-ai.ide.gateway`, account = the gateway's id. P2P-AI account sign-ins (phase A3)
keep their refresh token in the keychain too, under service `org.p2p-ai.ide.account`, account = the home's id.

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
| `gateways[].kind` | string | `local`, `manual`, `public` | `public` (picked from the public list, phase I2b): every connection to it goes only to public addresses (CWE-918); otherwise display only |
| `gateways[].ca_pem` | string, optional | PEM certificates only, max 64 KiB, no private key | when present, the only trusted root for that gateway (CWE-295) |
| `gateways[].account` | object, optional | `{home, audience}`: `home` as in `accounts.json`, `audience` 3-128 chars `a-z 0-9 . _ : -` (starting with a letter or digit) | phase A3: the gateway signs in with this P2P-AI account instead of its key; never on a `tandemforge` entry; holds no secret |
| `active` | string, optional | an id in the list | which gateway chat uses |

Limits: 32 gateways, file 4 MiB. Change it through the Gateways view, not by hand.

## accounts.json

Phase A3 ([features/account-sign-in.md](../features/account-sign-in.md)): the account services the person signed in
at. No secret: refresh tokens are in the OS keychain, access tokens only in memory.

```json
{
  "homes": [
    { "id": "9c1e0b7a55d2f310", "issuer": "https://accounts.example.org", "email": "dana@example.org",
      "password_change_required": false, "signed_in_at": 1795400000 }
  ]
}
```

| Key | Type | Default | Allowed values | Security impact |
|---|---|---|---|---|
| `homes[].id` | string | none | must equal the first 16 hex of SHA-256 of `issuer` | names the keychain entry; checked on load so an edited file cannot point an entry at another home (CWE-20) |
| `homes[].issuer` | string | none | `https://host[:port][/path]`, or `http://127.0.0.1:port` (INSECURE — DEV ONLY), normalised, no trailing slash | where the refresh token is sent; must equal the home's `iss` (RFC 9207, NIST SC-8) |
| `homes[].email` | string | `""` | up to 254 characters, no control characters | display only |
| `homes[].password_change_required` | bool | `false` | | shows "choose your own password" |
| `homes[].signed_in_at` | number | `0` | Unix seconds | display only |

Limits: 16 homes, file 256 KiB. Re-checked on every load; a bad entry refuses the whole file.

## directories.json

The directories the public gateway list is fetched from (phase I2b, [features/public-gateways.md](../features/public-gateways.md)).
Empty or missing by default: the IDE ships with no directory and fetches nothing until the person adds one.

```json
{
  "directories": ["https://directory.example.org", "https://dir2.example.org:8443"]
}
```

| Key | Type | Default | Allowed values | Security impact |
|---|---|---|---|---|
| `directories` | array of strings | `[]` | 0-8 entries; each `https://` to a public host (no localhost, private/link-local IPv4, IPv6 literal, user info, query or fragment), stored without a trailing slash, no duplicates | where adverts are fetched from; a private target would be SSRF (CWE-918). A directory can hide gateways but cannot forge one (adverts are signed) |

Re-checked on every load; a bad entry refuses the whole file. Change it in the Gateways view. Each entry was added by
the person; record who requested a directory and when if the list is managed for others.

## links.json

Which gateway project belongs to an open folder (phase I4, [features/missions.md](../features/missions.md)). Written
when the person picks or creates a project in the Missions view; missing means no links.

```json
{
  "links": [
    { "folder": "9b1c0e5d2f4a6b7c8d9e0f1a2b3c4d5e", "gateway_id": "3f2a9c0d1e4b5a67",
      "project_id": "prj_example", "project_name": "shop-api" }
  ]
}
```

| Key | Type | Allowed values | Security impact |
|---|---|---|---|
| `links[].folder` | string | 32 hex: the first half of the SHA-256 of the folder's path | the folder path itself is not stored (CWE-359) |
| `links[].gateway_id` | string | an allow-listed id | which saved gateway the link is for |
| `links[].project_id` | string | an allow-listed id (letters, digits, `_`, `-`) | used in `/v1/projects/{id}/...` paths; the allow-list keeps it to one path segment (CWE-20, CWE-918) |
| `links[].project_name` | string | up to 128 chars, control characters removed | display only |

Limits: 200 links (oldest dropped), file 256 KiB; unknown keys refuse the file. One folder can be linked to one
project per gateway. Change it in the Missions view.

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
| `messages[]` | `{role, content, at}` | role `user`, `assistant`, `error` (a local error line) or `tool` (a tool step line, phase I3); `error` and `tool` lines are never sent to a model; up to 400 messages |

Limits: 500 chats per folder, 4 MiB per chat. Delete chats in the History list (two clicks); deleting the folder
removes all history.

**Privacy:** chats can contain code you pasted and the model's answers. They stay on this computer; the IDE does not
upload them anywhere except the messages you send to the gateway you chose.

## run-jobs/ and run-cache/ (phase I5c local sandbox, phase I5d container)

* `run-jobs/<32 hex>/`: one folder per local sandbox run, created fresh and owner-only (0700), holding the copy of
  the open folder plus the run's `HOME` and `TMPDIR`. It is removed when the run ends; a folder left after a crash
  can be deleted safely.
* `run-cache/<24 hex>/`: build caches kept between runs of the same folder so a second run does not rebuild
  everything: `target` (Cargo), `go-build`, `gopath`, `npm`, `xdg`, and `cargo-home` (its `registry` and `git` are
  links to `~/.cargo`, which the sandbox can only read). The name is the start of the folder key (SHA-256 of the folder's path, as in `links.json`), so the path
  itself is not in the name (CWE-359). Delete the whole folder to free space; the next run rebuilds.

* `run-grants.json` (Windows only): the folders outside the data folder that the running job's sandbox SID was given
  read-and-execute on, as `[{"path": ..., "sid": "S-1-15-2-..."}]`. Written before each grant and deleted when the
  run ends; if the IDE crashed mid-run, the next local run removes the listed grants first. Only `S-1-15-2-` SIDs
  are ever removed, so an edited file cannot strip the person's own access. Safe to leave alone.

On Windows the job folder and the cache are also shared with the folder's sandbox SID (modify), so the AppContainer
can use them; nothing else in the data folder is.

The container runner (I5d) uses the same two folders: the job folder is mounted at `/work` and the cache at
`/cache` (with `:Z` on SELinux hosts, which relabels only these folders). Its job folders also hold `.p2pide-home`
and `.p2pide-tmp`. Images are kept by the engine, not here.

**Security:** both folders are writable by sandboxed and container runs, which run untrusted project code. Nothing in them is
trusted by the IDE; it never runs or loads files from them. See [features/local-sandbox.md](../features/local-sandbox.md).

## extensions/ (phase I6a)

Extensions installed from Open VSX ([features/extensions.md](../features/extensions.md)).

| Path | What it holds |
|---|---|
| `extensions/installed.json` | The installed list: one object per extension (below). Written by the core with an atomic replace |
| `extensions/<id>-<version>/` | The unpacked `extension/` folder of the verified `.vsix` (for example `dracula-theme.theme-dracula-2.25.1/`) |
| `extensions/.staging-<random>/` | A download being unpacked; renamed into place when every entry passed the checks, removed on failure |

`installed.json` keys:

| Key | Type | Description |
|---|---|---|
| `id` | string | `namespace.name`, lowercase; allow-listed characters only |
| `namespace`, `name`, `version` | string | As published on Open VSX (allow-listed) |
| `display_name`, `description` | string | From the manifest, size-capped, control characters removed. Shown as text only |
| `kind` | `declarative`, `web` or `node` | What the extension needs to run. Phase I6a installs only `declarative` |
| `enabled` | bool | Turned on in the Extensions view. A turned-off extension gives the editor nothing |
| `installed_at` | integer | Unix seconds |
| `sha256` | string | SHA-256 of the verified `.vsix`, hex |
| `verified_publisher` | bool | Open VSX marks the namespace as verified |
| `folder` | string | The folder name above; checked again before any read |

Security: the files are third-party data that passed the hash and signature checks at install time; the editor reads
them only through the core, only for enabled extensions, and only inside each extension's own folder (cap-std,
CWE-22). Deleting the `extensions/` folder removes every extension; nothing else refers to it.

The chosen colour theme is kept in the webview's local storage under `p2p-ide.colorTheme` (a theme id, no secrets).

