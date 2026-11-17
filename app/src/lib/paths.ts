/**
 * lib/paths.ts -- small pure helpers for workspace-relative paths and editor languages.
 *
 * Purpose: shared by the explorer, tabs and editor; kept free of React and Tauri so node --test can check them.
 * Trust boundary: none. These helpers only format paths the Rust core already returned; the core, not this file,
 * enforces path confinement (CWE-22), so nothing here is a security control.
 */

/** The last part of a `/`-separated relative path ("src/main.py" -> "main.py"). */
export function baseName(rel: string): string {
  const i = rel.lastIndexOf("/");
  return i < 0 ? rel : rel.slice(i + 1);
}

/** The folder part of a relative path ("src/main.py" -> "src", "a.txt" -> ""). */
export function dirName(rel: string): string {
  const i = rel.lastIndexOf("/");
  return i < 0 ? "" : rel.slice(0, i);
}

/** Breadcrumb parts for the editor header ("src/a/b.ts" -> ["src", "a", "b.ts"]). */
export function crumbs(rel: string): string[] {
  return rel === "" ? [] : rel.split("/");
}

/** Monaco language id per file extension or file name; "plaintext" when unknown. */
const BY_EXT: Record<string, string> = {
  ts: "typescript", tsx: "typescript", mts: "typescript", cts: "typescript",
  js: "javascript", jsx: "javascript", mjs: "javascript", cjs: "javascript",
  json: "json", jsonc: "json", md: "markdown", markdown: "markdown",
  py: "python", rs: "rust", go: "go", java: "java", kt: "kotlin", c: "c", h: "c",
  cpp: "cpp", cc: "cpp", hpp: "cpp", cs: "csharp", rb: "ruby", php: "php", swift: "swift",
  sh: "shell", bash: "shell", ps1: "powershell", psm1: "powershell", bat: "bat", cmd: "bat",
  yml: "yaml", yaml: "yaml", toml: "ini", ini: "ini", cfg: "ini", xml: "xml", svg: "xml",
  html: "html", htm: "html", css: "css", scss: "scss", less: "less", sql: "sql", lua: "lua",
  dockerfile: "dockerfile",
};
const BY_NAME: Record<string, string> = { Dockerfile: "dockerfile", Makefile: "plaintext", ".gitignore": "plaintext" };

/** Picks the Monaco language for a file path. */
export function languageFor(rel: string): string {
  const name = baseName(rel);
  if (BY_NAME[name]) return BY_NAME[name];
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "plaintext";
  return BY_EXT[name.slice(dot + 1).toLowerCase()] ?? "plaintext";
}

/** A short size label for the explorer's tooltip (bytes, KiB or MiB). */
export function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
