// vite.config.ts -- builds the IDE's webview files into dist/ for Tauri. Dev server on 127.0.0.1 only (never on the
// LAN). React's JSX is compiled by Vite's own Oxc transform (automatic runtime), so no React plugin is needed;
// Tailwind CSS 4 runs as a Vite plugin (pinned in package.json). No source maps in the build (nothing internal shipped).
//
// SECURITY (phase I6a): "monaco-editor" is now an alias of @codingame/monaco-vscode-editor-api, whose sanitizer
// imports the dompurify package itself; the npm override in package.json pins that to the patched 3.4.18 (CWE-79).
// The I1 plugin that redirected monaco-editor 0.57.0's vendored DOMPurify 3.4.15 is no longer needed and was removed.
import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import tailwindcss from "@tailwindcss/vite";

/**
 * Phase I6b: the page's own, stricter CSP as a <meta> tag (build only; the dev server needs inline HMR scripts).
 *
 * The CSP in src-tauri/tauri.conf.json is sent with every HTML file Tauri serves, including the hidden iframe of
 * the web worker extension host, which must evaluate extension code ('unsafe-eval', as VS Code for the web does).
 * A browser applies every policy a document has, so this tag takes 'unsafe-eval' away from the IDE's main page
 * again: the main page keeps the I1 policy (no eval), and only the sandboxed iframe and its worker may evaluate.
 * SECURITY: derived from the one shipped CSP, so the two can't drift; frame-ancestors is header-only and is dropped
 * here (browsers ignore it in a meta tag). ASVS V14.4, CWE-95.
 */
function pageCsp(): Plugin {
  const csp: string = JSON.parse(readFileSync(new URL("./src-tauri/tauri.conf.json", import.meta.url), "utf8")).app.security.csp;
  const page = csp
    .split(";")
    .map((d) => d.trim())
    .filter((d) => d && !d.startsWith("frame-ancestors"))
    .map((d) => (d.startsWith("script-src") ? d.replace(/ 'unsafe-eval'/g, "") : d))
    .join("; ");
  if (page.includes("'unsafe-eval'")) throw new Error("the main page CSP must not allow eval");
  return {
    name: "ide-page-csp",
    apply: "build",
    transformIndexHtml: () => [{ tag: "meta", attrs: { "http-equiv": "Content-Security-Policy", content: page }, injectTo: "head-prepend" }],
  };
}

export default defineConfig({
  clearScreen: false,
  plugins: [pageCsp(), tailwindcss()],
  oxc: { jsx: { runtime: "automatic" } },
  server: { host: "127.0.0.1", port: 5174, strictPort: true },
  // The editor and TextMate workers are built as separate files so the CSP's worker-src 'self' covers them.
  worker: { format: "es" },
  build: { outDir: "dist", target: "es2022", sourcemap: false, chunkSizeWarningLimit: 6000 },
});
