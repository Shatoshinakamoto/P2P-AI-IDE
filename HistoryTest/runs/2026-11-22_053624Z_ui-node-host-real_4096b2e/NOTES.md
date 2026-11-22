# Notes for 2026-11-22_053624Z_ui-node-host-real_4096b2e

## 2026-11-22 05:37 UTC

Correction (2026-11-22 05:45Z): this run's 'real Node host starts extensions' check read only the exthost1 log, which an earlier manual run on the same server wrote, so the check itself did not prove this run's connection. The server's exthost3 log from this run does show the same activations (git-base, git, github, debug-auto-launch, merge-conflict) and no error. The test now reads only logs created during the run; see the next ui-node-host-real run.
