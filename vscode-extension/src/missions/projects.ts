/**
 * projects.ts -- the open folder as a gateway project, for missions (side track X3). No `vscode` import, so it is
 * tested with plain Node.
 *
 * Purpose: a TypeScript port of the IDE's core/src/projects.rs (phase I4). Autopilot and Auto Improve run on a
 * gateway, on a *project* whose workspace holds a copy of the code. This module decides what to send (push plan),
 * what came back (pull plan), runs push and pull, and turns gateway answers into small fixed view shapes for the
 * Missions view. The rules are the IDE's, checked by the shared cases in core/tests/vectors/missions.json (Rust:
 * core/src/projects_vectors.rs).
 *
 * Inputs / trust boundaries:
 *  - the open folder, read through `Workspace` (links never followed);
 *  - gateway answers (untrusted): every field is picked by name, typed, length-capped and stripped of control
 *    characters before it reaches the view, which shows it as text only (CWE-20, CWE-79, CWE-150);
 *  - ids from the view (project, mission, proposal, run, approval): checked against a fixed pattern before they go
 *    into a URL path (CWE-74, CWE-918).
 *
 * Security controls:
 *  - Secrets stay home: the agent deny list applies to push, and those names are never pulled back (CWE-200).
 *  - Only names the gateway's path grammar accepts are sent (ASCII subset, no Windows device names, no case
 *    collisions); others are listed as skipped (CWE-22, CWE-178).
 *  - Caps: 2000 files, 5 MiB per file, 100 MiB per push; 50 files per pull (CWE-400, CWE-770).
 *  - Each file is re-read and re-hashed just before upload; commits carry `base_version` (CWE-362, CWE-367).
 *  - Downloads must match the listed SHA-256 (NIST SI-7, CWE-345) and be UTF-8 text; pulled files only become
 *    pending changes the person accepts per file (OWASP LLM06).
 * Dependencies: ../agent/{tools,workspace}.ts, ../gateway/{checks,client}.ts.
 */
import { NetError } from "../gateway/checks.ts";
import { encodePath } from "../gateway/client.ts";
import { denied, proposeExternal, type Pending } from "../agent/tools.ts";
import { versionOf, type Workspace } from "../agent/workspace.ts";

export const MAX_PUSH_FILES = 2000;
export const MAX_PUSH_FILE = 5 << 20;
export const MAX_PUSH_TOTAL = 100 << 20;
export const MAX_PULL_FILES = 50;
export const MAX_COMMIT_CHANGES = 1000;

const DEVICE_NAMES = ["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];

/** Whether a relative path is safe to send: the IDE's ASCII subset of the gateway's workspace path grammar. */
export function gatewayPathOk(rel: string): boolean {
  if (typeof rel !== "string" || rel === "" || rel.length > 512) return false;
  const segs = rel.split("/");
  if (segs.length > 32) return false;
  return segs.every((s) => {
    const base = (s.split(".")[0] ?? "").trimEnd().toLowerCase();
    return (
      s !== "" &&
      s.length <= 255 &&
      s !== "." &&
      s !== ".." &&
      !s.startsWith(" ") &&
      !s.endsWith(" ") &&
      !s.endsWith(".") &&
      /^[A-Za-z0-9 \-_.()+@,=~]+$/.test(s) &&
      !DEVICE_NAMES.includes(base)
    );
  });
}

/** An id that goes into a URL path: 1-64 of `[A-Za-z0-9_-]` (CWE-74). */
export function idOk(id: unknown): id is string {
  return typeof id === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(id);
}

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const get = (v: unknown, k: string): unknown => (isObj(v) ? v[k] : undefined);

/** Rust's `char::is_control` (Unicode Cc): U+0000-001F and U+007F-009F. */
const CONTROL = /[\x00-\x1f\x7f-\x9f]/;

/** Text from a gateway answer, made safe to show: control characters removed except newline, cut to `max` characters. */
export function clean(v: unknown, key: string, max: number): string {
  const s = get(v, key);
  if (typeof s !== "string") return "";
  return [...s].filter((c) => c === "\n" || !CONTROL.test(c)).slice(0, max).join("");
}
/** A whole number from a gateway answer (Rust `as_i64`), else 0. */
export function num(v: unknown, key: string): number {
  const n = get(v, key);
  return typeof n === "number" && Number.isSafeInteger(n) ? n : 0;
}
const flag = (v: unknown, key: string): boolean => get(v, key) === true;
const arr = (v: unknown, key: string): unknown[] => {
  const a = get(v, key);
  return Array.isArray(a) ? a : [];
};

export interface RemoteFile {
  path: string;
  bytes: number;
  sha256: string;
}

/** Parses `GET .../workspace/files`, keeping only well-formed entries. */
export function parseRemoteFiles(v: unknown): RemoteFile[] {
  const out: RemoteFile[] = [];
  for (const f of arr(v, "data").slice(0, 10_000)) {
    const path = get(f, "path");
    const sha = get(f, "sha256");
    if (typeof path !== "string" || typeof sha !== "string") continue;
    const s = sha.toLowerCase();
    if (!gatewayPathOk(path) || !/^[0-9a-f]{64}$/.test(s)) continue;
    const b = get(f, "bytes");
    out.push({ path, bytes: typeof b === "number" && Number.isSafeInteger(b) && b >= 0 ? b : 0, sha256: s });
  }
  return out;
}

export interface Skipped {
  rel: string;
  reason: string;
}

/** Picks which local files may be sent, before any is read: deny list, grammar, size, case collisions, caps. */
export function eligible(files: [string, number][]): { ok: string[]; skipped: Skipped[] } {
  const ok: string[] = [];
  const skipped: Skipped[] = [];
  const folded = new Set<string>();
  let total = 0;
  for (const [rel, size] of files) {
    let reason: string | undefined;
    // SECURITY: likely secrets are never sent (CWE-200).
    if (denied(rel)) reason = "may hold secrets, kept on this computer";
    else if (!gatewayPathOk(rel)) reason = "name the gateway does not accept";
    else if (size > MAX_PUSH_FILE) reason = "larger than 5 MiB";
    else if (folded.has(rel.toLowerCase())) reason = "differs from another name only by case";
    else if (ok.length >= MAX_PUSH_FILES || total + size > MAX_PUSH_TOTAL) reason = "over the push limit (2000 files, 100 MiB)";
    // Rust inserts into the case set before the cap check, so a capped name still counts for later collisions.
    if (reason === undefined || reason.startsWith("over the push")) folded.add(rel.toLowerCase());
    if (reason !== undefined) skipped.push({ rel, reason });
    else {
      total += size;
      ok.push(rel);
    }
  }
  return { ok, skipped };
}

export interface PushPlan {
  upload: [string, string][];
  delete: string[];
  unchanged: number;
}

/** Compares hashed local files with the gateway's list. Skipped local paths keep their gateway copies. */
export function planPush(local: [string, string][], remote: RemoteFile[], skippedLocal: Skipped[]): PushPlan {
  const remoteMap = new Map(remote.map((r) => [r.path, r.sha256]));
  const plan: PushPlan = { upload: [], delete: [], unchanged: 0 };
  const localSet = new Set<string>();
  for (const [rel, sha] of local) {
    localSet.add(rel);
    if (remoteMap.get(rel) === sha) plan.unchanged++;
    else plan.upload.push([rel, sha]);
  }
  const kept = new Set(skippedLocal.map((s) => s.rel));
  for (const r of remote) if (!localSet.has(r.path) && !kept.has(r.path)) plan.delete.push(r.path);
  return plan;
}

export interface PullItem {
  path: string;
  sha256: string;
  localVersion: string | undefined;
}
export interface PullPlan {
  fetch: PullItem[];
  same: number;
  onlyLocal: number;
  skipped: Skipped[];
}

/** Compares the gateway's files with local versions and lists what differs. */
export function planPull(local: [string, string][], remote: RemoteFile[]): PullPlan {
  const localMap = new Map(local);
  const plan: PullPlan = { fetch: [], same: 0, onlyLocal: 0, skipped: [] };
  const remoteSet = new Set(remote.map((r) => r.path));
  for (const r of remote) {
    const lv = localMap.get(r.path);
    if (lv === r.sha256) {
      plan.same++;
      continue;
    }
    let reason: string | undefined;
    // SECURITY: a gateway cannot plant a file where the person keeps secrets.
    if (denied(r.path)) reason = "a name kept private; not brought back";
    else if (r.bytes > MAX_PUSH_FILE) reason = "larger than 5 MiB";
    else if (plan.fetch.length >= MAX_PULL_FILES) reason = "over the limit of 50 files per pull; pull again after reviewing";
    if (reason !== undefined) plan.skipped.push({ rel: r.path, reason });
    else plan.fetch.push({ path: r.path, sha256: r.sha256, localVersion: lv });
  }
  plan.onlyLocal = local.filter(([r]) => !remoteSet.has(r)).length;
  return plan;
}

/** Checks downloaded bytes against the listed digest and makes them text. Returns the text or a reason. */
export function checkDownload(item: PullItem, bytes: Uint8Array): { text: string } | { error: string } {
  if (versionOf(bytes) !== item.sha256) return { error: "changed on the gateway while downloading; pull again" };
  try {
    return { text: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
  } catch {
    return { error: "not a text file; get it from the gateway's web app" };
  }
}

/** The gateway calls push and pull need (GatewayClient, or a fake in tests). */
export interface ProjectApi {
  api(method: "GET" | "POST" | "PUT" | "DELETE", path: string, body?: unknown): Promise<unknown>;
  uploadFile(name: string, bytes: Uint8Array): Promise<unknown>;
  download(path: string): Promise<Buffer>;
}

export interface PushReport {
  uploaded: number;
  deleted: number;
  unchanged: number;
  skipped: Skipped[];
  version: number;
  copy: string;
}
export interface PullReport {
  proposed: number;
  same: number;
  onlyLocal: number;
  skipped: Skipped[];
}

const filesPath = (pid: string): string => `/v1/projects/${pid}/workspace/files`;

/** Errors that stop a whole push or pull, as opposed to one file being refused. */
function fatal(e: unknown): boolean {
  if (!(e instanceof NetError)) return true;
  return !(e.kind === "gateway" || e.kind === "bad_request" || (e.kind === "status" && [400, 413, 415].includes(Number(e.detail))));
}
const fileReason = (e: unknown): string => (e instanceof NetError && (e.kind === "gateway" || e.kind === "bad_request") ? e.message : "the gateway refused this file");

/** Waits (up to 60 s) until uploaded files leave quarantine, when the gateway scans uploads. */
async function waitScanned(api: ProjectApi, ids: string[], sleep: (ms: number) => Promise<void>): Promise<void> {
  const t0 = Date.now();
  for (const id of ids) {
    for (;;) {
      const state = get(await api.api("GET", `/v1/files/${id}`), "state");
      if (state === "quarantined" && Date.now() - t0 < 60_000) await sleep(2000);
      else if (state === "quarantined") throw new NetError("gateway", "a file is still being scanned on the gateway; try again shortly");
      else if (state === "rejected") throw new NetError("gateway", "the gateway's malware scan rejected a file");
      else break;
    }
  }
}

/**
 * Sends the open folder to the gateway project: uploads new and changed files, deletes files that are gone, commits
 * them as the next version, then asks the gateway to copy the project to a node.
 */
export async function push(api: ProjectApi, ws: Workspace, pid: string, sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms))): Promise<PushReport> {
  if (!idOk(pid)) throw new NetError("bad_request", "pick a gateway project first");
  let version = num(await api.api("GET", `/v1/projects/${pid}/workspace`), "version");
  const remote = parseRemoteFiles(await api.api("GET", filesPath(pid)));
  const all = ws.allFiles(MAX_PUSH_FILES * 4);
  const { ok, skipped } = eligible(all.files);
  if (all.truncated) skipped.push({ rel: "(rest of the folder)", reason: "the folder has too many files; only the first are sent" });
  const local: [string, string][] = [];
  for (const rel of ok) {
    try {
      local.push([rel, ws.readBytes(rel, MAX_PUSH_FILE).version]);
    } catch {
      skipped.push({ rel, reason: "could not be read" });
    }
  }
  const plan = planPush(local, remote, skipped);
  const changes: unknown[] = [];
  const quarantined: string[] = [];
  for (const [rel, sha] of plan.upload) {
    let read;
    try {
      read = ws.readBytes(rel, MAX_PUSH_FILE);
    } catch {
      skipped.push({ rel, reason: "could not be read" });
      continue;
    }
    // SECURITY: re-hashed right before upload; a file changed meanwhile is not sent half-checked (CWE-367).
    if (read.version !== sha) {
      skipped.push({ rel, reason: "changed while sending; push again" });
      continue;
    }
    try {
      const f = await api.uploadFile(rel.split("/").pop() as string, read.bytes);
      const id = clean(f, "id", 64);
      if (!idOk(id)) throw new NetError("bad_answer");
      if (get(f, "state") === "quarantined") quarantined.push(id);
      changes.push({ path: rel, file_id: id });
    } catch (e) {
      if (fatal(e)) throw e;
      skipped.push({ rel, reason: fileReason(e) });
    }
  }
  const uploaded = changes.length;
  for (const d of plan.delete) changes.push({ path: d, delete: true });
  await waitScanned(api, quarantined, sleep);
  for (let i = 0; i < changes.length; i += MAX_COMMIT_CHANGES) {
    // SECURITY: base_version, so a concurrent change on the gateway is refused, never overwritten (CWE-362).
    const r = await api.api("POST", `/v1/projects/${pid}/workspace/commit`, { base_version: version, changes: changes.slice(i, i + MAX_COMMIT_CHANGES) });
    version = num(r, "version");
  }
  let copy: string;
  try {
    copy = clean(await api.api("POST", `/v1/projects/${pid}/workspace/sync`, {}), "state", 16);
  } catch (e) {
    if (!(e instanceof NetError)) throw e;
    if (e.kind === "not_offered" || (e.kind === "status" && Number(e.detail) === 503) || (e.kind === "gateway" && e.message === "this gateway has that feature switched off")) copy = "off";
    else if (e.kind === "gateway") copy = e.message;
    else throw e;
  }
  return { uploaded, deleted: plan.delete.length, unchanged: plan.unchanged, skipped, version, copy };
}

/**
 * Brings the gateway project's files back as pending changes for review.
 * SECURITY: private names never come back; each download must match its listed SHA-256 and be text; every file
 * waits for the person's Accept (OWASP LLM06, NIST SI-7).
 */
export async function pull(api: ProjectApi, ws: Workspace, pending: Pending, pid: string): Promise<PullReport> {
  if (!idOk(pid)) throw new NetError("bad_request", "pick a gateway project first");
  const remote = parseRemoteFiles(await api.api("GET", filesPath(pid)));
  const local: [string, string][] = [];
  for (const r of remote) {
    try {
      local.push([r.path, ws.readBytes(r.path, MAX_PUSH_FILE).version]);
    } catch {
      /* not here, or not readable: treated as new */
    }
  }
  const plan = planPull(local, remote);
  const remoteSet = new Set(remote.map((r) => r.path));
  plan.onlyLocal = eligible(ws.allFiles(MAX_PUSH_FILES * 4).files).ok.filter((r) => !remoteSet.has(r)).length;
  const report: PullReport = { proposed: 0, same: plan.same, onlyLocal: plan.onlyLocal, skipped: plan.skipped };
  for (const item of plan.fetch) {
    let bytes: Buffer;
    try {
      bytes = await api.download(`${filesPath(pid)}/${encodePath(item.path)}?download=1`);
    } catch (e) {
      if (fatal(e)) throw e;
      report.skipped.push({ rel: item.path, reason: fileReason(e) });
      continue;
    }
    const t = checkDownload(item, bytes);
    if ("error" in t) {
      report.skipped.push({ rel: item.path, reason: t.error });
      continue;
    }
    if (proposeExternal(ws, pending, item.path, item.localVersion, t.text) === undefined) report.proposed++;
    else report.skipped.push({ rel: item.path, reason: "could not be added for review (changed locally, or too many waiting)" });
  }
  return report;
}

/** The key for a folder in the saved links: first 32 hex of SHA-256 of its real path (the path is not stored). */
export function folderKey(root: string): string {
  return versionOf(root).slice(0, 32);
}

// ---- view shapes (all from untrusted gateway JSON)

export interface ProjectView {
  id: string;
  name: string;
  role: string;
  version: number;
  file_count: number;
}
export function projectView(v: unknown): ProjectView | undefined {
  const id = clean(v, "id", 64);
  return idOk(id) ? { id, name: clean(v, "name", 128), role: clean(v, "role", 16), version: num(v, "version"), file_count: num(v, "file_count") } : undefined;
}

export interface Switches {
  autopilot_available: boolean;
  improve_available: boolean;
  disabled: boolean;
  autopilot: boolean;
  auto_improve: boolean;
  resource_scope: string;
}
export function switches(exp: unknown, flags: unknown): Switches {
  return {
    autopilot_available: flag(get(exp, "autopilot"), "available"),
    improve_available: flag(get(exp, "auto_improve"), "available"),
    disabled: flag(exp, "disabled"),
    autopilot: flag(flags, "autopilot"),
    auto_improve: flag(flags, "auto_improve"),
    resource_scope: clean(flags, "resource_scope", 16),
  };
}

export interface SyncView {
  state: string;
  version: number;
  delta_state: string;
  delta_version: number;
  delta_added: number;
  delta_modified: number;
  delta_deleted: number;
  delta_conflicts: string[];
}
export function syncView(v: unknown): SyncView {
  const d = get(v, "delta") ?? null;
  return {
    state: clean(v, "state", 16),
    version: num(v, "version"),
    delta_state: clean(d, "state", 16),
    delta_version: num(d, "version"),
    delta_added: num(d, "added"),
    delta_modified: num(d, "modified"),
    delta_deleted: num(d, "deleted"),
    delta_conflicts: arr(d, "conflicts")
      .slice(0, 20)
      .filter((p): p is string => typeof p === "string" && gatewayPathOk(p)),
  };
}

export interface MissionView {
  id: string;
  kind: string;
  project_id: string;
  model: string;
  goal: string;
  state: string;
  reason: string;
  cycle: number;
  max_cycles: number;
  tokens: number;
  run_id: string;
  waiting_approval: boolean;
  checks_passed: number;
  checks_total: number;
  tasks_done: number;
  tasks_total: number;
  proposals_created: number;
  updated_at: number;
  tasks: { title: string; state: string }[];
  checks: { id: string; passed: boolean; exit_code: number; error: string }[];
}
export function missionView(v: unknown): MissionView | undefined {
  const id = clean(v, "id", 64);
  if (!idOk(id)) return undefined;
  const run = clean(v, "run_id", 64);
  return {
    id,
    kind: clean(v, "kind", 16),
    project_id: clean(v, "project_id", 64),
    model: clean(v, "model", 200),
    goal: clean(v, "goal", 1000),
    state: clean(v, "state", 24),
    reason: clean(v, "reason", 300),
    cycle: num(v, "cycle"),
    max_cycles: num(v, "max_cycles"),
    tokens: num(v, "tokens"),
    run_id: idOk(run) ? run : "",
    waiting_approval: flag(v, "waiting_approval"),
    checks_passed: num(v, "checks_passed"),
    checks_total: num(v, "checks_total"),
    tasks_done: num(v, "tasks_done"),
    tasks_total: num(v, "tasks_total"),
    proposals_created: num(v, "proposals_created"),
    updated_at: num(v, "updated_at"),
    tasks: arr(v, "tasks").slice(0, 64).map((t) => ({ title: clean(t, "title", 300), state: clean(t, "state", 16) })),
    checks: arr(v, "checks").slice(0, 8).map((c) => ({ id: clean(c, "id", 32), passed: flag(c, "passed"), exit_code: num(c, "exit_code"), error: clean(c, "error", 64) })),
  };
}

/** Object keys sorted (byte order), like serde_json's default map, so both apps show the same text. */
function sortKeys(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (!isObj(v)) return v;
  // SECURITY: a null-prototype object, so a "__proto__" key from the gateway stays plain data (CWE-1321).
  const out: Obj = Object.create(null);
  for (const k of Object.keys(v).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))) out[k] = sortKeys(v[k]);
  return out;
}

export interface ApprovalView {
  id: string;
  tool: string;
  access: string;
  /** The call's arguments as pretty JSON text (shown verbatim, never run), at most 64 Ki characters. */
  arguments: string;
  arguments_sha256: string;
  expires_at: number;
}
export function approvalView(v: unknown): ApprovalView | undefined {
  const id = clean(v, "id", 64);
  const sha = clean(v, "arguments_sha256", 64);
  if (!idOk(id) || !/^[0-9a-fA-F]{64}$/.test(sha)) return undefined;
  const a = get(v, "arguments");
  const args = a === undefined ? "" : JSON.stringify(sortKeys(a), null, 2);
  return { id, tool: clean(v, "tool", 64), access: clean(v, "access", 16), arguments: [...args].slice(0, 64 << 10).join(""), arguments_sha256: sha, expires_at: num(v, "expires_at") };
}

export interface ProposalView {
  id: string;
  title: string;
  kind: string;
  rationale: string;
  effort: string;
  steps: string[];
  sources: string[];
  state: string;
}
export function proposalView(v: unknown): ProposalView | undefined {
  const id = clean(v, "id", 64);
  if (!idOk(id)) return undefined;
  const strings = (k: string, n: number, max: number): string[] =>
    arr(v, k)
      .slice(0, n)
      .filter((s): s is string => typeof s === "string")
      .map((s) => [...s].filter((c) => !CONTROL.test(c)).slice(0, max).join(""));
  return {
    id,
    title: clean(v, "title", 200),
    kind: clean(v, "kind", 16),
    rationale: clean(v, "rationale", 2000),
    effort: clean(v, "effort", 32),
    steps: strings("steps", 12, 300),
    // Sources are plain text, never links.
    sources: strings("sources", 8, 300),
    state: clean(v, "state", 16),
  };
}

/** Test frameworks the gateway knows. */
export const FRAMEWORKS = ["cargo", "go", "make", "npm", "pytest", "unittest"];

export interface CheckInput {
  id: string;
  kind: string;
  framework?: string;
  argv?: string[];
}

/**
 * Turns the start form's checks into the gateway's `acceptance` list (same rules as the IDE).
 * SECURITY: argv is an array (no shell, CWE-78); the gateway checks programs against its own allow-list too.
 */
export function acceptance(checks: CheckInput[]): unknown[] | { error: string } {
  if (!Array.isArray(checks) || checks.length === 0 || checks.length > 8) return { error: "Autopilot needs 1 to 8 checks that say when the work is done" };
  const out: unknown[] = [];
  const ids = new Set<string>();
  for (const c of checks) {
    if (!isObj(c) || typeof c.id !== "string" || !/^[a-z0-9_-]{1,32}$/.test(c.id) || ids.has(c.id)) {
      return { error: "check names must be unique: lower-case letters, digits, - and _" };
    }
    ids.add(c.id);
    const fw = c.framework ?? "";
    const argv = c.argv ?? [];
    if (c.kind === "tests" && typeof fw === "string" && FRAMEWORKS.includes(fw)) out.push({ id: c.id, kind: "tests", framework: fw });
    else if (
      c.kind === "command" &&
      Array.isArray(argv) &&
      argv.length > 0 &&
      argv.length <= 32 &&
      argv.every((a) => typeof a === "string" && a !== "" && Buffer.byteLength(a) <= 512 && !CONTROL.test(a))
    ) {
      out.push({ id: c.id, kind: "command", argv });
    } else return { error: "each check is a test framework from the list, or a command split into words" };
  }
  return out;
}
