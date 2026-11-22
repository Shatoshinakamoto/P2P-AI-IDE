/**
 * lib/vscodeVersion.ts -- does the Node extension host speak the editor's protocol? (phase I6c-3,
 * docs/features/node-extensions-design.md).
 *
 * Purpose: the editor (the @codingame/monaco-vscode-api library) and the Node extension host server (VSCodium) are
 * both built from VS Code. They talk over VS Code's numbered message protocol, whose numbering changes between VS Code
 * minor versions. A mismatched pair connects without any error but never starts an extension (found on the PC: an
 * editor on 1.145 with a 1.140 server). So the editor offers and connects Node extensions only when both are built
 * from the same VS Code minor version.
 *
 * Inputs: version strings, from the editor library's product data and from the shell (the pinned server's VS Code
 * version). Pure functions, no side effects.
 * Security: fail closed: a version that does not parse never matches (CWE-1188 style safe default), so a malformed
 * value can't turn the check off.
 */

/** `major.minor` of a version like `1.145.2` or `1.140.0`, or null when it doesn't start with two numbers. */
export function minorOf(version: string): string | null {
  // SECURITY: allow-list format, digits only, bounded length (CWE-20).
  const m = /^(\d{1,4})\.(\d{1,5})(?:\.|$)/.exec(version.trim());
  return m ? `${Number(m[1])}.${Number(m[2])}` : null;
}

/** True when both versions parse and have the same `major.minor`. */
export function sameMinor(editor: string, server: string): boolean {
  const a = minorOf(editor);
  const b = minorOf(server);
  return a !== null && a === b;
}
