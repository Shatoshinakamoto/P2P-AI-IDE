/**
 * monaco.ts -- loads Monaco (the editor core of VS Code, MIT) with its language workers and the IDE's dark theme.
 *
 * Purpose: one import point so the editor, the workers and the theme are set up exactly once.
 * Security: workers are bundled files (no blob: or remote code), matching the CSP's worker-src 'self'. Monaco's own
 * DOMPurify copy is replaced by the patched package at build time (vite.config.ts, CWE-79). Monaco never fetches
 * anything from the network here: no CDN loader is used.
 * Dependencies: monaco-editor 0.57.0 (pinned). VS Code extension support (@codingame/monaco-vscode-api) replaces
 * this setup in phase I6.
 */
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import JsonWorker from "monaco-editor/language/json/json.worker.js?worker";
import CssWorker from "monaco-editor/language/css/css.worker.js?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker.js?worker";
import TsWorker from "monaco-editor/language/typescript/ts.worker.js?worker";

// Monaco asks this hook for a worker per language service.
self.MonacoEnvironment = {
  getWorker(_id: string, label: string): Worker {
    if (label === "json") return new JsonWorker();
    if (label === "css" || label === "scss" || label === "less") return new CssWorker();
    if (label === "html" || label === "handlebars" || label === "razor") return new HtmlWorker();
    if (label === "typescript" || label === "javascript") return new TsWorker();
    return new EditorWorker();
  },
};

// A theme close to VS Code's Dark Modern, so the layout feels familiar.
monaco.editor.defineTheme("p2p-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#1f1f1f",
    "editorGutter.background": "#1f1f1f",
    "editorLineNumber.foreground": "#6e7681",
    "editorLineNumber.activeForeground": "#cccccc",
  },
});

export { monaco };
