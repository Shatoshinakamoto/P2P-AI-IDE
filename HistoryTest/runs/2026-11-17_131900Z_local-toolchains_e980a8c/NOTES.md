# Notes for 2026-11-17_131900Z_local-toolchains_e980a8c

## 2026-11-17 13:46 UTC

2026-11-17 correction: the PASS line 'a second cargo run reuses the per-folder build cache' was checked by run time under 15 s and no 'Compiling calc' in stderr, but cargo ran with --quiet, which hides that line, so the check could not see a recompile. A later Windows run showed the project crate is rebuilt each run (fresh copy at a new path). The other 5 results stand; the cache claim is replaced by a check that build output lands in the cache (from commit after 8870edc).
