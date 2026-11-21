// vite.config.ts -- builds the IDE's webview files into dist/ for Tauri. Dev server on 127.0.0.1 only (never on the
// LAN). React's JSX is compiled by Vite's own Oxc transform (automatic runtime), so no React plugin is needed;
// Tailwind CSS 4 runs as a Vite plugin (pinned in package.json). No source maps in the build (nothing internal shipped).
//
// SECURITY (phase I6a): "monaco-editor" is now an alias of @codingame/monaco-vscode-editor-api, whose sanitizer
// imports the dompurify package itself; the npm override in package.json pins that to the patched 3.4.18 (CWE-79).
// The I1 plugin that redirected monaco-editor 0.57.0's vendored DOMPurify 3.4.15 is no longer needed and was removed.
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  clearScreen: false,
  plugins: [tailwindcss()],
  oxc: { jsx: { runtime: "automatic" } },
  server: { host: "127.0.0.1", port: 5174, strictPort: true },
  // The editor and TextMate workers are built as separate files so the CSP's worker-src 'self' covers them.
  worker: { format: "es" },
  build: { outDir: "dist", target: "es2022", sourcemap: false, chunkSizeWarningLimit: 6000 },
});
