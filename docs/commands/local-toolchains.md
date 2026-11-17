# `cargo run --example local_toolchains`: live check of real toolchains in the local sandbox

Development test only; not part of the app. Added 2026-11-17 for phase I5c
([features/local-sandbox.md](../features/local-sandbox.md)), to prove that toolchain mode lets real build and test
tools run inside the confinement.

## Purpose

`core/src/localbox.rs` tests the confinement itself with python3 (no network, home hidden, writes kept in the job
folder, leftover processes gone). This example makes one tiny project per framework, each with one passing and one
failing test, and runs it through `localbox::run` exactly as the Run and test view does:

| Framework | Project | Expected |
|---|---|---|
| cargo (twice) | a library with 2 tests, 1 failing | verdict fail, 1 passed, 1 failed, the failing test named; cargo's build output is in the per-folder cache, not in the copy (run times printed as INFO) |
| go | a module with 2 tests, 1 failing | as above |
| npm | `node --test` through `npm test` | as above |
| make | a `test` target that fails | verdict fail |
| unittest | `python3 -m unittest discover` | as above |

## Synopsis

```
cd core && cargo run --example local_toolchains
```

No options, no environment variables read (the sandbox builds its own environment). No root needed; the computer
needs Linux with Landlock and unprivileged user namespaces. A framework whose program is not installed prints
`SKIP` and does not count as passed.

## Output and exit codes

One `PASS name`, `FAIL name: reason` or `SKIP name: reason` line per check, then a last line
`{"passed": n, "failed": n, "skipped": n}`. Exit code 0 only when nothing failed; 1 otherwise.

## Security

Everything runs inside the sandbox under test, on throw-away folders in the temp folder that are removed at the end.
The projects have no dependencies, so no network is needed (and none is reachable).

## Examples

```
cd core && cargo run --example local_toolchains            # all frameworks
cd core && cargo run --example local_toolchains 2>/dev/null | tail -1   # summary line only
```
