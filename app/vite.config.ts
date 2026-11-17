// vite.config.ts -- builds the IDE's webview files into dist/ for Tauri. Dev server on 127.0.0.1 only (never on the
// LAN). React's JSX is compiled by Vite's own Oxc transform (automatic runtime), so no React plugin is needed;
// Tailwind CSS 4 runs as a Vite plugin (pinned in package.json). No source maps in the build (nothing internal shipped).
//
// SECURITY: monaco-editor 0.57.0 vendors DOMPurify 3.4.15 (esm/vs/base/browser/dompurify/dompurify.js), which has
// published advisories fixed in 3.4.17+ (GitHub Advisory Database, GHSA-p98j-92pf-mc4p and others; see
// docs/SECURITY-LOG.md 2026-11-17). The plugin below swaps that vendored copy for the pinned dompurify 3.4.18 package,
// so the sanitizer Monaco runs on hover and markdown content is the patched one (CWE-79, CWE-1395).
import { defineConfig, type Plugin } from "vite";
import tailwindcss from "@tailwindcss/vite";
/** Redirects Monaco's vendored DOMPurify import to the patched package; touches no other import. */
function patchedDompurify(): Plugin {
  return {
    name: "p2p-ide-patched-dompurify",
    enforce: "pre",
    async resolveId(source, importer) {
      // SECURITY: match only Monaco's own relative import from its domSanitize module.
      if (
        source === "./dompurify/dompurify.js" &&
        importer !== undefined &&
        importer.replaceAll("\\", "/").includes("/monaco-editor/esm/vs/base/browser/")
      ) {
        // Resolve the package's own ES module entry (its "exports" map), i.e. the pinned 3.4.18 in node_modules.
        return this.resolve("dompurify", importer, { skipSelf: true });
      }
      return null;
    },
  };
}

export default defineConfig({
  clearScreen: false,
  plugins: [patchedDompurify(), tailwindcss()],
  oxc: { jsx: { runtime: "automatic" } },
  server: { host: "127.0.0.1", port: 5174, strictPort: true },
  // Monaco's language workers are built as separate files so the CSP's worker-src 'self' covers them.
  worker: { format: "es" },
  build: { outDir: "dist", target: "es2022", sourcemap: false, chunkSizeWarningLimit: 6000 },
});
