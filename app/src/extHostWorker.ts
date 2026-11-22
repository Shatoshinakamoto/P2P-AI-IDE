/**
 * extHostWorker.ts -- the web worker extension host's entry module (phase I6b).
 *
 * Purpose: loads extNetShim.ts first, so `fetch` is replaced before VS Code's extension host and any extension code
 * run, then VS Code's own worker. ES module imports are evaluated in order, so the shim always runs first.
 * Security: see extNetShim.ts (internet only for extensions the person allowed, through the Rust core).
 */
import "./extNetShim.ts";
import "@codingame/monaco-vscode-api/workers/extensionHost.worker";
