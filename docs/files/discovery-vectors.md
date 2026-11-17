# Shared advert vectors (`core/tests/vectors/discovery-vectors.json`)

**What it is:** a copy of the P2P-AI protocol's shared test vectors, used by `core/tests/discovery.rs` to prove the
IDE verifies gateway advertisements byte-for-byte like the P2P-AI node and the TypeScript protocol package.

**Read by:** the core's tests only. Product code never reads it, and it is not shipped.

**Source:** P2P-AI repository, `decentralized/protocol/vectors/vectors.json`, last changed in commit `a96a294`.
Copied 2026-11-17 for phase I2b. SHA-256 of the copy:
`f67d07e873a2206c383c242155ed63da1fcd241173791b092f4f4395fe80ca2c`.

## Entries

| Key | What it is | Used for |
|---|---|---|
| `seed_hex` | the ed25519 test seed (32 bytes, hex). A public test value, never a real key | signing synthetic adverts and probe replies in the tests |
| `now` | the test clock (seconds) | verifying at a fixed time |
| `advert` | a node-facing signed advert | must verify; canonical form must reproduce it; listed as `not_for_users` |
| `probe` | a signed probe reply for that advert | must verify; wrong nonce or stale time must not |
| `advert_reach` | an advert reachable through a relay (ADR-026) | must verify |
| `advert_reach_refused` | 11 relay-rule violations, each with its expected code | each must be refused with that code |
| `advert_client` | an advert with `client_access` (ADR-028) | must verify; its access fields are checked |
| `advert_client_refused` | 14 `client_access` violations, each with its expected code | each must be refused with that code |

## How to refresh it

1. In the P2P-AI repository, check the protocol package's own tests pass (`decentralized/protocol`).
2. Copy the file over this one, update the commit and SHA-256 above, and run `cargo test` in `core/`.
3. A failure means the protocol changed: port the change to `core/src/discovery.rs` (and check the node's
   `P2P-AI-Nodes/Rust/src/discovery.rs` did the same) before merging.

Changes need review by whoever owns the protocol in P2P-AI, because the copy defines what the IDE accepts.

**Risk if wrong:** a stale or edited copy could let the tests pass while the IDE accepts adverts the protocol
refuses (or the reverse). The provenance line and hash make a silent change visible in review.
