/**
 * workspace.ts -- the open folder as the extension's agent tools see it (side track X2). No `vscode` import, so it
 * is tested with plain Node.
 *
 * Purpose: list, read, search and write files in one folder, with the same rules and limits as the IDE's
 * core/src/workspace.rs. The chat's agent tools (./tools.ts) only touch the disk through this class.
 *
 * Inputs / trust boundary: the folder is the first folder of the person's (trusted) VS Code workspace. Every relative
 * path comes from model output and is untrusted: it is checked here before any file system call. File contents are
 * untrusted data.
 *
 * Security controls:
 *  - Path allow-list (same as the Rust `join_checked`; shared cases in core/tests/vectors/agent_tools.json): plain
 *    `/`-separated components only; no absolute paths, `.`/`..`, empty parts, backslashes, colons (Windows drive
 *    letters, alternate data streams), NUL or other control characters; at most 1024 bytes (CWE-22, CWE-73).
 *  - No links at all: every component of the path is checked with `lstat`, and a symlink or junction anywhere in it
 *    is refused; the last component is also opened with O_NOFOLLOW where the OS has it. This is stricter than the
 *    IDE, which follows links that stay inside (CWE-59).
 *  - Type and size are taken from the open file handle, and the read is bounded (CWE-367, CWE-400). Caps: 5 MiB per
 *    file, 5000 entries per listing, search 200 matches / 20 000 files / 1 MiB per file / 256-byte query.
 *  - Writes are atomic (temp file with the `wx` flag in the same folder, flushed, then renamed) and check a SHA-256
 *    version, so an accept never overwrites a change made meanwhile (CWE-367 lost update). SHA-256 is an approved
 *    hash (FIPS 180-4).
 *  - Search is a plain substring match, never a regular expression (CWE-1333 ReDoS).
 *  - Errors are fixed short messages without paths or OS text (CWE-209).
 * SECURITY-WARNING: Node has no `openat`/RESOLVE_BENEATH, so the per-component `lstat` check and the open are two
 * steps. A program that already runs as the person and swaps a folder for a link between them could redirect one
 * read or write. Accepted: such a program can already change the files directly. TODO(security): use a handle-based
 * walk if Node gains one.
 * Dependencies: Node's built-in `node:fs`, `node:path`, `node:crypto` only.
 */
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";

export const MAX_FILE_BYTES = 5 << 20;
export const MAX_DIR_ENTRIES = 5000;
export const MAX_REL_PATH_BYTES = 1024;
export const MAX_SEARCH_RESULTS = 200;
export const MAX_QUERY_BYTES = 256;
export const MAX_SEARCH_FILE_BYTES = 1 << 20;
export const MAX_SEARCH_FILES = 20_000;
/** Folders search skips (build and dependency output), same as the IDE. */
export const SEARCH_SKIP_DIRS = [".git", "node_modules", "target", "dist", ".venv", "__pycache__"];

export type WsErrorKind =
  | "bad_root"
  | "bad_path"
  | "outside"
  | "not_found"
  | "wrong_kind"
  | "too_large"
  | "not_text"
  | "conflict"
  | "bad_query"
  | "io";

/** Same wording as core/src/workspace.rs `WsError::message`. Never holds a path or OS text (CWE-209). */
const MESSAGES: Record<WsErrorKind, string> = {
  bad_root: "choose an existing folder (not a whole drive)",
  bad_path: "that file name is not allowed",
  outside: "that path leads outside the workspace folder",
  not_found: "file or folder not found",
  wrong_kind: "that is a folder, not a file (or the reverse)",
  too_large: "the file is larger than 5 MiB, so it is not opened in the editor",
  not_text: "the file is not UTF-8 text, so it is not opened in the editor",
  conflict: "the file changed on disk since you opened it; reload it before saving",
  bad_query: "enter 1 to 256 characters to search",
  io: "the file system refused the operation",
};

export class WsError extends Error {
  readonly kind: WsErrorKind;
  constructor(kind: WsErrorKind) {
    super(MESSAGES[kind]);
    this.name = "WsError";
    this.kind = kind;
  }
}

/** Maps a Node error to a fixed kind (never its message, which holds the path). */
function ioErr(e: unknown): WsError {
  if (e instanceof WsError) return e;
  const code = (e as { code?: string })?.code ?? "";
  if (code === "ENOENT" || code === "ENOTDIR") return new WsError("not_found");
  if (code === "ELOOP") return new WsError("outside"); // O_NOFOLLOW on a link
  if (code === "EISDIR") return new WsError("wrong_kind");
  return new WsError("io");
}

/** SHA-256 of bytes as lowercase hex: the file "version" used to detect outside changes. */
export function versionOf(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

const utf8 = new TextEncoder();

/**
 * Checks a relative path against the allow-list without touching the disk. Returns its parts (`[]` for the root).
 * SECURITY: see the file header (CWE-22, CWE-73). @throws WsError `bad_path`.
 */
export function checkRel(rel: string): string[] {
  if (typeof rel !== "string" || utf8.encode(rel).length > MAX_REL_PATH_BYTES) throw new WsError("bad_path");
  if (rel === "") return [];
  // SECURITY: characters that change meaning on some OS are refused before looking at parts.
  if (/[\\:\x00-\x1f\x7f-\x9f]/.test(rel)) throw new WsError("bad_path");
  const parts = rel.split("/");
  for (const p of parts) {
    if (p === "" || p === "." || p === "..") throw new WsError("bad_path");
  }
  return parts;
}

export type EntryKind = "file" | "dir" | "symlink";
export interface Entry {
  name: string;
  rel: string;
  kind: EntryKind;
  size: number;
}
export interface FileText {
  text: string;
  version: string;
}
export interface Match {
  rel: string;
  line: number;
  preview: string;
}

const O_NOFOLLOW = fs.constants.O_NOFOLLOW ?? 0;
let tmpCounter = 0;

export class Workspace {
  /** The folder's real path. */
  readonly root: string;

  private constructor(root: string) {
    this.root = root;
  }

  /**
   * Opens a folder. SECURITY: must be an absolute path to an existing folder and not a filesystem or drive root
   * (far too broad for the agent, NIST AC-6); the real path is kept so later checks compare real paths (CWE-59).
   * @throws WsError `bad_root`.
   */
  static open(folder: string): Workspace {
    if (!path.isAbsolute(folder)) throw new WsError("bad_root");
    let real: string;
    try {
      real = fs.realpathSync.native(folder);
      if (!fs.statSync(real).isDirectory()) throw new WsError("bad_root");
    } catch {
      throw new WsError("bad_root");
    }
    if (path.parse(real).root === real) throw new WsError("bad_root");
    return new Workspace(real);
  }

  /** The folder's name, for the chat view. */
  name(): string {
    return path.basename(this.root);
  }

  /**
   * Resolves checked parts to an absolute path, refusing any link on the way (the last part only if `lastMayBeMissing`
   * is false and it exists). SECURITY: `lstat` on every component, so no link or junction is ever followed (CWE-59).
   */
  private resolve(parts: string[], allowMissingLast: boolean): string {
    let cur = this.root;
    for (let i = 0; i < parts.length; i++) {
      cur = path.join(cur, parts[i]);
      let st: fs.Stats;
      try {
        st = fs.lstatSync(cur);
      } catch (e) {
        if (allowMissingLast && i === parts.length - 1 && (e as { code?: string }).code === "ENOENT") return cur;
        throw ioErr(e);
      }
      if (st.isSymbolicLink()) throw new WsError("outside");
      if (i < parts.length - 1 && !st.isDirectory()) throw new WsError("wrong_kind");
    }
    return cur;
  }

  /** Lists one folder, folders first, then files, each sorted by name (case-insensitive). Links are shown, not followed. */
  listDir(rel: string): { entries: Entry[]; truncated: boolean } {
    const parts = checkRel(rel);
    const dir = this.resolve(parts, false);
    let names: fs.Dirent[];
    try {
      if (!fs.lstatSync(dir).isDirectory()) throw new WsError("wrong_kind");
      names = fs.readdirSync(dir, { withFileTypes: true });
    } catch (e) {
      throw ioErr(e);
    }
    const entries: Entry[] = [];
    let truncated = false;
    for (const d of names) {
      if (entries.length >= MAX_DIR_ENTRIES) {
        truncated = true;
        break;
      }
      // Names a later call would refuse are left out (for example a name with a colon or backslash).
      const childRel = rel === "" ? d.name : `${rel}/${d.name}`;
      try {
        checkRel(childRel);
      } catch {
        continue;
      }
      const kind: EntryKind = d.isSymbolicLink() ? "symlink" : d.isDirectory() ? "dir" : d.isFile() ? "file" : "symlink";
      let size = 0;
      if (kind === "file") {
        try {
          size = fs.lstatSync(path.join(dir, d.name)).size;
        } catch {
          continue;
        }
      }
      entries.push({ name: d.name, rel: childRel, kind, size });
    }
    const rank = (k: EntryKind): number => (k === "dir" ? 0 : 1);
    entries.sort((a, b) => rank(a.kind) - rank(b.kind) || a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    return { entries, truncated };
  }

  /** Reads at most `max` bytes of a file. SECURITY: no links; type and size from the open handle; bounded (CWE-367, CWE-400). */
  readBytes(rel: string, max = MAX_FILE_BYTES): { bytes: Buffer; version: string } {
    const parts = checkRel(rel);
    if (parts.length === 0) throw new WsError("wrong_kind");
    const p = this.resolve(parts, false);
    let fd: number;
    try {
      fd = fs.openSync(p, fs.constants.O_RDONLY | O_NOFOLLOW);
    } catch (e) {
      throw ioErr(e);
    }
    try {
      const st = fs.fstatSync(fd);
      if (!st.isFile()) throw new WsError("wrong_kind");
      if (st.size > max) throw new WsError("too_large");
      // SECURITY: read at most max + 1 bytes even if the file grows after the size check (CWE-400).
      const buf = Buffer.alloc(Math.min(st.size, max) + 1);
      let n = 0;
      for (;;) {
        const r = fs.readSync(fd, buf, n, buf.length - n, null);
        if (r === 0) break;
        n += r;
        if (n >= buf.length) break;
      }
      if (n > max) throw new WsError("too_large");
      const bytes = buf.subarray(0, n);
      return { bytes, version: versionOf(bytes) };
    } catch (e) {
      throw ioErr(e);
    } finally {
      fs.closeSync(fd);
    }
  }

  /** Reads a UTF-8 text file and its version. @throws WsError `not_text` for anything that is not UTF-8. */
  readText(rel: string): FileText {
    const { bytes, version } = this.readBytes(rel);
    let text: string;
    try {
      text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch {
      throw new WsError("not_text");
    }
    return { text, version };
  }

  /**
   * Writes a text file atomically. `baseVersion` is the version the change is based on, or undefined for a new file.
   * Returns the new version.
   * SECURITY: never through a link (CWE-59); lost-update check (CWE-367): an existing file must still have
   * `baseVersion`, and a new file must still not exist; temp file created with `wx` (no clobbering), flushed, then
   * renamed over the target. @throws WsError `conflict`, `outside`, `wrong_kind`, `too_large`, `not_found`, `io`.
   */
  writeText(rel: string, text: string, baseVersion: string | undefined): string {
    const bytes = Buffer.from(text, "utf8");
    if (bytes.length > MAX_FILE_BYTES) throw new WsError("too_large");
    const parts = checkRel(rel);
    if (parts.length === 0) throw new WsError("wrong_kind");
    const parent = this.resolve(parts.slice(0, -1), false);
    try {
      if (!fs.lstatSync(parent).isDirectory()) throw new WsError("wrong_kind");
    } catch (e) {
      throw ioErr(e);
    }
    const target = path.join(parent, parts[parts.length - 1]);
    let exists = true;
    try {
      const st = fs.lstatSync(target);
      if (st.isSymbolicLink()) throw new WsError("outside");
      if (!st.isFile()) throw new WsError("wrong_kind");
    } catch (e) {
      if ((e as { code?: string }).code === "ENOENT") exists = false;
      else throw ioErr(e);
    }
    if (exists) {
      if (baseVersion === undefined || this.readText(rel).version !== baseVersion) throw new WsError("conflict");
    } else if (baseVersion !== undefined) {
      throw new WsError("conflict"); // it was deleted meanwhile
    }
    const short = [...parts[parts.length - 1]].slice(0, 64).join("");
    const tmp = path.join(parent, `.${short}.p2pai-${process.pid}-${Date.now()}-${tmpCounter++}.tmp`);
    try {
      // SECURITY: `wx` fails if the name exists (also a link), so the temp file is ours.
      const fd = fs.openSync(tmp, "wx", 0o644);
      try {
        fs.writeSync(fd, bytes);
        fs.fsyncSync(fd);
      } finally {
        fs.closeSync(fd);
      }
      if (!exists) {
        // A new file: refuse if one appeared meanwhile (link() fails on an existing name, unlike rename()).
        try {
          fs.linkSync(tmp, target);
          fs.unlinkSync(tmp);
        } catch (e) {
          const code = (e as { code?: string }).code ?? "";
          if (code !== "EPERM" && code !== "ENOTSUP" && code !== "ENOSYS" && code !== "EOPNOTSUPP") throw e;
          // Some file systems (FAT, some network shares) have no hard links: check once more, then rename.
          if (fs.existsSync(target)) throw Object.assign(new Error("exists"), { code: "EEXIST" });
          fs.renameSync(tmp, target);
        }
      } else {
        fs.renameSync(tmp, target);
      }
    } catch (e) {
      try {
        fs.unlinkSync(tmp);
      } catch {
        /* already gone */
      }
      if ((e as { code?: string }).code === "EEXIST") throw new WsError("conflict");
      throw ioErr(e);
    }
    return versionOf(bytes);
  }

  /**
   * Every file under the folder as `[rel, size]`, sorted, skipping build and dependency folders and links (X3 push;
   * same as the IDE's `all_files`). Returns `truncated` when more than `maxFiles` exist (CWE-400).
   */
  allFiles(maxFiles: number): { files: [string, number][]; truncated: boolean } {
    const out: [string, number][] = [];
    const stack = [""];
    while (stack.length > 0) {
      const dirRel = stack.pop() as string;
      let listing: Entry[];
      try {
        listing = this.listDir(dirRel).entries;
      } catch {
        continue;
      }
      for (const e of listing) {
        if (e.kind === "dir" && !SEARCH_SKIP_DIRS.includes(e.name)) stack.push(e.rel);
        else if (e.kind === "file") {
          if (out.length >= maxFiles) return { files: out, truncated: true };
          out.push([e.rel, e.size]);
        }
      }
    }
    out.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
    return { files: out, truncated: false };
  }

  /**
   * Searches text files for a plain substring, case-insensitive. Skips build folders, links, files over 1 MiB and
   * non-UTF-8 files; stops at 200 matches or 20 000 files (CWE-400, CWE-1333).
   */
  search(query: string): { matches: Match[]; truncated: boolean } {
    if (typeof query !== "string" || query === "" || utf8.encode(query).length > MAX_QUERY_BYTES || /[\x00-\x1f\x7f-\x9f]/.test(query)) {
      throw new WsError("bad_query");
    }
    const needle = query.toLowerCase();
    const matches: Match[] = [];
    let truncated = false;
    let seen = 0;
    const stack = [""];
    walk: while (stack.length > 0) {
      const dirRel = stack.pop() as string;
      let listing: Entry[];
      try {
        listing = this.listDir(dirRel).entries;
      } catch {
        continue;
      }
      for (const e of listing) {
        if (e.kind === "dir") {
          if (!SEARCH_SKIP_DIRS.includes(e.name)) stack.push(e.rel);
          continue;
        }
        if (e.kind !== "file") continue;
        if (++seen > MAX_SEARCH_FILES) {
          truncated = true;
          break walk;
        }
        if (e.size > MAX_SEARCH_FILE_BYTES) continue;
        let text: string;
        try {
          text = this.readText(e.rel).text;
        } catch {
          continue;
        }
        const lines = text.split("\n");
        for (let i = 0; i < lines.length; i++) {
          const line = lines[i].replace(/\r$/, "");
          if (line.toLowerCase().includes(needle)) {
            if (matches.length >= MAX_SEARCH_RESULTS) {
              truncated = true;
              break walk;
            }
            matches.push({ rel: e.rel, line: i + 1, preview: [...line.trim()].slice(0, 200).join("") });
          }
        }
      }
    }
    matches.sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : a.line - b.line));
    return { matches, truncated };
  }
}
