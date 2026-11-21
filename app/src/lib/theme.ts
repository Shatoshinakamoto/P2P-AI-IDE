/**
 * theme.ts -- remembers the colour theme the person picked (phase I6a), on this computer only.
 *
 * Security: localStorage holds only the theme id (no secrets); the value is checked for shape before use and any
 * storage error is ignored, so a blocked or cleared store just means the default theme (CWE-20).
 */
const KEY = "p2p-ide.colorTheme";

/** The saved theme id, or null. */
export function savedTheme(): string | null {
  try {
    const v = localStorage.getItem(KEY);
    return v && v.length <= 200 && !/[\u0000-\u001f]/.test(v) ? v : null;
  } catch {
    return null;
  }
}

export function saveTheme(id: string): void {
  try {
    localStorage.setItem(KEY, id);
  } catch {
    /* not saved; the default is used next time */
  }
}
