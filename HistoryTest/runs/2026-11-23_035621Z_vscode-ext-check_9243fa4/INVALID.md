# This run is marked INVALID: 2026-11-23_035621Z_vscode-ext-check_9243fa4

## 2026-11-23 03:57 UTC

Reason: The test result itself is correct, but the manifest chain link is wrong: this clone had core.autocrlf=true, so the previous run's manifest was hashed as its CRLF working copy (c91e8cdc...) instead of the committed LF file (f5a89e4b...). verify on any LF checkout would report a broken chain. The run was recorded again on an LF checkout right after this one.

The files of the run are kept as they were recorded.
