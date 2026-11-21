# Notes for 2026-11-21_205630Z_tf-engine-e2e_1e284ce

## 2026-11-21 20:57 UTC

Counts read from the log: the IDE core test passed 1 of 1 with 6 PASS steps, and all 10 backend checks in steps.txt gave the expected answers (401, 401, 403, 0600 files, digest only, engine.json removed on revoke, 4 chats to the local Ollama only, code not in the backend log). run.sh itself exits 0 even when the cargo test fails, so the verdict was checked by reading 'test result: ok' in the log.
