# Notes for 2026-11-22_114456Z_vscode-ext-integration_ae1f4aa

## 2026-11-22 11:56 UTC

Failed at suite.cjs:210 (switches: 'pick a gateway project first'). Cause found by repeating the run: on VS Code 1.147 a globalState write (the new project link) was undone a moment later, about 1 run in 9; VS Code 1.95.0 passed 15 of 15. Fixed in the next commit by keeping the extension's settings in its own state file; see the vscode-ext-integration runs at that commit (stable run repeated 20 times with 0 failures before recording).
