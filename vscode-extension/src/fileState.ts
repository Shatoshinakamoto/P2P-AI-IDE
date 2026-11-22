/**
 * fileState.ts -- the extension's saved settings in one JSON file of its own (side track X3).
 *
 * Purpose: keeps the saved gateways (names, addresses, pinned CAs; never keys), the active gateway, the picked model
 * per gateway, the Tools switch and the folder-to-project links in `state.json` in the extension's global storage
 * folder. These were in VS Code's `globalState` in X1 and X2; the X3 integration test showed that on VS Code
 * 1.147 a write there was sometimes undone a moment later (about 1 run in 9: a removed gateway came back, a new link
 * vanished), when an older copy of the extension's state came back from VS Code's main process. VS Code 1.95
 * never did it in 15 runs. A file of our own, read fresh on every get and written atomically, has no such race.
 *
 * Inputs / trust boundary: the file is ours, but another program or a hand edit could change it, so every reader
 * re-checks what it gets (GatewayStore re-validates each gateway, the links are re-checked).
 * Security controls:
 *  - Keys never go here: they stay in SecretStorage (CWE-312, CWE-522).
 *  - Size cap on read (1 MiB) and a JSON object check; anything else reads as empty (CWE-400, CWE-20).
 *  - Atomic write: a temp file with a random name created with `wx` and mode 0600, then renamed over the file, so a
 *    crash never leaves half a file and nothing is written through a planted link (CWE-367, CWE-59). The folder is
 *    created with mode 0700 (CWE-276).
 *  - Keys are an allow-list of names; a "__proto__" key from the file stays plain data (null-prototype object,
 *    CWE-1321).
 *  - One-time move of the old `globalState` values into the file on first use.
 * Dependencies: Node's built-in `node:fs` and `node:path`; the `vscode` API (types and the global storage folder).
 */
import * as fs from "node:fs";
import * as path from "node:path";
import type * as vscode from "vscode";

const FILE = "state.json";
const MAX_BYTES = 1 << 20;

export class FileState {
  private readonly dir: string;
  private readonly file: string;

  constructor(ctx: vscode.ExtensionContext, migrateKeys: readonly string[] = []) {
    this.dir = ctx.globalStorageUri.fsPath;
    this.file = path.join(this.dir, FILE);
    // One-time move from globalState (X1/X2 kept these there).
    if (!fs.existsSync(this.file)) {
      const old: Record<string, unknown> = Object.create(null);
      for (const k of ctx.globalState.keys()) {
        if (migrateKeys.some((p) => k === p || (p.endsWith(".") && k.startsWith(p)))) old[k] = ctx.globalState.get(k);
      }
      this.write(old);
    }
  }

  /** The whole file as an object (empty when missing, too large or broken). */
  private read(): Record<string, unknown> {
    try {
      if (fs.statSync(this.file).size > MAX_BYTES) return Object.create(null);
      const v: unknown = JSON.parse(fs.readFileSync(this.file, "utf8"));
      if (typeof v !== "object" || v === null || Array.isArray(v)) return Object.create(null);
      return Object.assign(Object.create(null), v);
    } catch {
      return Object.create(null);
    }
  }

  private write(v: Record<string, unknown>): void {
    fs.mkdirSync(this.dir, { recursive: true, mode: 0o700 });
    const rnd = Buffer.from(crypto.getRandomValues(new Uint8Array(8))).toString("hex");
    const tmp = path.join(this.dir, `.${FILE}.${rnd}.tmp`);
    try {
      fs.writeFileSync(tmp, JSON.stringify(v, null, 2), { flag: "wx", mode: 0o600 });
      fs.renameSync(tmp, this.file);
    } catch (e) {
      fs.rmSync(tmp, { force: true });
      throw e;
    }
  }

  get<T>(key: string): T | undefined {
    return this.read()[key] as T | undefined;
  }

  /** Sets one key (undefined removes it): a fresh read, the change, then an atomic write. */
  async update(key: string, value: unknown): Promise<void> {
    const v = this.read();
    if (value === undefined) delete v[key];
    else v[key] = value;
    this.write(v);
  }
}
