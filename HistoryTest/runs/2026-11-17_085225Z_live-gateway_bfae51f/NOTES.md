# Notes for 2026-11-17_085225Z_live-gateway_bfae51f

## 2026-11-17 08:53 UTC

Counts from the log's SUMMARY line: 19 passed, 0 failed (16 from the IDE core example, 3 from the driver). The recorder did not parse this format. Earlier unrecorded tries on the same day found two test problems, not product bugs: unittest discovery skipped tests/ without __init__.py (mission looped until the key's token cap), and the key's request limiter answered the duplicate-mission probe first (the IDE correctly showed the rate-limit message); the test project and probe were fixed before this run.
