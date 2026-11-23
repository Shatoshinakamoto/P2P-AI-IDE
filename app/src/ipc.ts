/**
 * ipc.ts -- typed wrappers for the Tauri commands in src-tauri/src/main.rs (files) and net.rs (gateways, chat).
 *
 * Purpose: one place where the webview talks to the Rust core, with the record shapes it returns.
 * Trust boundary: the webview is untrusted to the core; the core checks every argument (paths, sizes, queries).
 * Errors arrive as short fixed strings safe to show (CWE-209). Nothing here reads files or the network itself.
 */
import { Channel, invoke } from "@tauri-apps/api/core";

export type EntryKind = "file" | "dir" | "symlink";
export interface Entry { name: string; rel: string; kind: EntryKind; size: number }
export interface Listing { entries: Entry[]; truncated: boolean }
export interface FileText { text: string; version: string }
export interface Match { rel: string; line: number; preview: string }
export interface SearchResult { matches: Match[]; truncated: boolean }
export interface WorkspaceInfo { name: string }

/** Turns any rejection into a display string. */
function msg(e: unknown): Error {
  return new Error(typeof e === "string" ? e : "the app could not finish that");
}

/** Opens the OS folder picker; resolves to null when the person cancels. */
export async function pickFolder(): Promise<WorkspaceInfo | null> {
  try { return await invoke<WorkspaceInfo | null>("pick_folder"); } catch (e) { throw msg(e); }
}

/** The open workspace, if any (after a reload of the webview). */
export async function workspaceInfo(): Promise<WorkspaceInfo | null> {
  try { return await invoke<WorkspaceInfo | null>("workspace_info"); } catch (e) { throw msg(e); }
}

/** Lists one folder ("" = root). */
export async function listDir(rel: string): Promise<Listing> {
  try { return await invoke<Listing>("list_dir", { rel }); } catch (e) { throw msg(e); }
}

/** Reads a text file with its version. */
export async function readFile(rel: string): Promise<FileText> {
  try { return await invoke<FileText>("read_file", { rel }); } catch (e) { throw msg(e); }
}

/** Saves a file; baseVersion is the loaded version (null creates a new file). Resolves to the new version. */
export async function writeFile(rel: string, text: string, baseVersion: string | null): Promise<string> {
  try { return await invoke<string>("write_file", { rel, text, baseVersion }); } catch (e) { throw msg(e); }
}

/** Plain-text search across the workspace. */
export async function search(query: string): Promise<SearchResult> {
  try { return await invoke<SearchResult>("search", { query }); } catch (e) { throw msg(e); }
}

// ---- Phase I2: gateways and chat (src-tauri/src/net.rs). Keys go in, never come out: only `has_key` is returned.

export interface GatewayView {
  id: string;
  name: string;
  url: string;
  kind: "local" | "manual" | "public" | "tandemforge" | "nexus";
  /** Plain http on loopback: shown as INSECURE - DEV ONLY. */
  dev: boolean;
  pinned: boolean;
  has_key: boolean;
  /** Phase A3: the P2P-AI account this gateway signs in with (takes the place of the key). */
  account: AccountLink | null;
}
export interface AccountLink { home: string; audience: string }
export interface GatewayList { gateways: GatewayView[]; active: string | null }
export interface GatewayCheck { latency_ms: number; models: string[] }
export interface Msg { role: "system" | "user" | "assistant"; content: string }
export interface StreamEnd { finish_reason: string | null; chars: number }
export interface ChatMessage { role: "user" | "assistant" | "error" | "tool"; content: string; at: number }
export interface Chat { id: string; title: string; gateway: string; model: string; created: number; updated: number; messages: ChatMessage[] }
export interface ChatSummary { id: string; title: string; updated: number; count: number }
export interface ChatHit { id: string; title: string; preview: string }
type ChatEvent = { kind: "delta"; text: string };
/** How people get access to a public gateway (from its signed advert, ADR-028 client_access). */
export interface ClientAccess {
  mode: "key" | "signup";
  api_url: string;
  signup_url: string;
  features: string[];
  terms_url: string;
  logs_prompts: boolean;
  retention_days: number;
}
/** One verified public gateway. Every string came from a signed advert: render it as text only (CWE-79). */
export interface PublicGateway {
  gateway_id: string;
  region: string;
  models: string[];
  access: ClientAccess;
  latency_ms: number | null;
  directories: number;
}
export interface PublicList { gateways: PublicGateway[]; rejected: Record<string, number> }

/** Calls a command and turns a rejection into an Error with the core's fixed message. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try { return await invoke<T>(cmd, args); } catch (e) { throw msg(e); }
}

export const gatewaysList = () => call<GatewayList>("gateways_list");
/** Adds a gateway by URL. The key (optional) goes straight to the OS keychain. */
export const gatewayAdd = (name: string, url: string, key: string | null) => call<string>("gateway_add", { name, url, key });
/** Adds the gateway set up on this computer; the person picks its setup folder in the OS dialog. */
export const gatewayAddLocal = (name: string, key: string | null) => call<string | null>("gateway_add_local", { name, key });
export const gatewayPickCa = (id: string) => call<boolean>("gateway_pick_ca", { id });
export const gatewayClearCa = (id: string) => call<void>("gateway_clear_ca", { id });
export const gatewaySetKey = (id: string, key: string) => call<void>("gateway_set_key", { id, key });
export const gatewayRemove = (id: string) => call<void>("gateway_remove", { id });
export const gatewaySetActive = (id: string) => call<void>("gateway_set_active", { id });
export const gatewayCheck = (id: string) => call<GatewayCheck>("gateway_check", { id });
// ---- Phase A3: sign in with a P2P-AI account (src-tauri/src/account.rs). Tokens never reach the webview.
export interface AccountHome { id: string; issuer: string; email: string; dev: boolean; signed_in: boolean; password_change_required: boolean }
export const accountsList = () => call<AccountHome[]>("accounts_list");
/** Opens the account service's sign-in page in the system browser; resolves when the browser comes back. */
export const accountSignIn = (home: string) => call<AccountHome>("account_sign_in", { home });
export const accountSignInCancel = () => call<void>("account_sign_in_cancel");
export const accountSignOut = (id: string) => call<void>("account_sign_out", { id });
export const accountChangePassword = (id: string, current: string, next: string) =>
  call<void>("account_change_password", { id, current, new: next });
/** Links a gateway to an account (home + the gateway's account id); an empty home goes back to the key. */
export const gatewaySetAccount = (id: string, home: string, audience: string) =>
  call<void>("gateway_set_account", { id, home, audience });
/** Phase I7: TandemForge on this computer. `running` means its engine file was found. */
export interface TandemForgeStatus { running: boolean; reason: string | null; id: string | null }
export const tandemforgeStatus = () => call<TandemForgeStatus>("tandemforge_status");
/** Pairs with the running TandemForge using the code it showed; the code goes to the OS keychain only. */
export const tandemforgeConnect = (code: string) => call<GatewayCheck>("tandemforge_connect", { code });

/** NEXUS Engine on this computer: nexus-proxy (11435) and nexus-runtime (11436). */
export type NexusRole = "proxy" | "runtime";
export interface NexusServiceStatus {
  service: "nexus-proxy" | "nexus-runtime";
  url: string;
  running: boolean;
  mock_upstream: boolean | null;
  airlock_threshold: number | null;
  active_model_mode: string | null;
  artifacts: number | null;
  episodic_memories: number | null;
  models_loaded: number | null;
  models_available: number | null;
  simd: string | null;
  id: string | null;
}
export interface NexusStatusView {
  proxy: NexusServiceStatus;
  runtime: NexusServiceStatus;
}
export const nexusStatus = () => call<NexusStatusView>("nexus_status");
/** Connects to a running local NEXUS Proxy or NEXUS Runtime instance (optional custom loopback port). */
export const nexusConnect = (role: NexusRole, port: number | null = null) =>
  call<GatewayCheck>("nexus_connect", { role, port });

/** The directories the public list is fetched from (phase I2b). */
export const directoriesList = () => call<string[]>("directories_list");
export const directoriesSet = (directories: string[]) => call<string[]>("directories_set", { directories });
/** Fetches, verifies (in Rust) and probes the public list. Sends no key. */
export const publicList = () => call<PublicList>("public_list");
/** Saves a gateway from the last list by its advert id; the core takes the address from the verified advert. */
export const gatewayAddPublic = (gatewayId: string, key: string | null) => call<string>("gateway_add_public", { gatewayId, key });

/**
 * Sends the conversation to the active gateway. `onText` gets each piece of the answer as it streams in.
 * SECURITY: answer text is untrusted; callers render it as text only (CWE-79).
 */
/**
 * A step of an agent turn: answer text, a tool that ran (name, path or query, ok), a run the assistant asks the person
 * to allow (phase I5b), or the full result of a run the person allowed.
 */
export type AgentEvent =
  | { kind: "delta"; text: string }
  | { kind: "tool"; name: string; target: string; ok: boolean }
  | { kind: "approval"; id: string; summary: string; runner: string }
  | { kind: "run"; id: string; result: RunResult };
export interface TurnEnd { steps: number; tool_calls: number; pending: number; finish_reason: string | null }
/** A file the assistant wants to change, waiting for review. */
export interface ChangeView { rel: string; created: boolean; added: number; removed: number }
export interface ChangeText { rel: string; before: string; after: string; created: boolean }

/**
 * Runs one agent turn with the folder tools (phase I3). Proposed edits are not written: they wait for review.
 * SECURITY: answer text and tool targets are untrusted; callers render them as text only (CWE-79).
 */
export async function agentSend(model: string, messages: Msg[], onEvent: (e: AgentEvent) => void, runs = false, runner = "gateway", image = "python"): Promise<TurnEnd> {
  const ch = new Channel<AgentEvent>();
  ch.onmessage = onEvent;
  return call<TurnEnd>("agent_send", { model, messages, runs, runner, image, onEvent: ch });
}
/** A runner the Run and test view can offer (phase I5c): `gateway`, `local-os` or `container`. */
export interface RunnerView { id: string; available: boolean; reason: string }
export const runnersList = () => call<RunnerView[]>("runners_list");
/** The person's answer to a run request on the approval card (phase I5b). Only the waiting request's id counts. */
export const agentDecide = (id: string, allow: boolean) => call<void>("agent_decide", { id, allow });

/** What to run (phase I5b, docs/features/run-and-test.md): tests with a framework, or one program as words. */
export interface RunInput { kind: "tests" | "command"; framework?: string; argv?: string[]; cwd?: string; timeout_s?: number }
export interface TestSummary { passed: number; failed: number; skipped: number; total: number; failures: string[] }
/** A run's result. SECURITY: all text is program output (untrusted); render as text only (CWE-79). */
export interface RunResult {
  runner: string; place: string; summary: string; exit_code: number | null; timed_out: boolean; duration_ms: number;
  stdout: string; stderr: string; truncated: boolean; tests: TestSummary | null; error: string;
}
/** The test frameworks the sandbox knows (the same list as the gateway's run_tests). */
export const FRAMEWORKS = ["cargo", "go", "make", "npm", "pytest", "unittest"] as const;
/**
 * Runs tests or a command for the person on `runner` (`gateway` in phase I5b). `onStage` gets `sending`, `copying`
 * and `running` as each stage starts.
 */
export async function runStart(runner: string, input: RunInput, onStage: (s: string) => void, image = "python"): Promise<RunResult> {
  const ch = new Channel<string>();
  ch.onmessage = onStage;
  return call<RunResult>("run_start", { runner, input, image, onStage: ch });
}
/** A container image from the core's fixed, digest-pinned list (phase I5d); `get` says what Get image does. */
export interface ImageView { id: string; label: string; present: boolean; get: "pull" | "build" }
export const containerImages = () => call<ImageView[]>("container_images");
/** Downloads (pull) or builds one image when the person presses Get image; `onStage` gets `pulling` or `building`. */
export async function containerPrepare(id: string, onStage: (s: string) => void): Promise<void> {
  const ch = new Channel<string>();
  ch.onmessage = onStage;
  return call<void>("container_prepare", { id, onStage: ch });
}
/** The image a run uses on the container runner: a test framework picks its own, a command the chosen one. */
export function imageFor(kind: "tests" | "command", framework: string, chosen: string): string {
  if (kind === "command") return chosen;
  return ({ pytest: "python", unittest: "python", npm: "node", go: "go", make: "go", cargo: "rust" } as Record<string, string>)[framework] ?? "python";
}
/**
 * Splits a typed command line into words like the core's `runner::split_words`: spaces separate words and "..." keeps
 * spaces inside one. No other shell rules: `;`, `|`, `$` and `*` are plain text for the program.
 */
export function splitWords(line: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quoted = false;
  let any = false;
  for (const c of line) {
    if (c === '"') { quoted = !quoted; any = true; }
    else if (/\s/.test(c) && !quoted) { if (any) { out.push(cur); cur = ""; any = false; } }
    else { cur += c; any = true; }
  }
  if (any) out.push(cur);
  return out;
}
export const changesList = () => call<ChangeView[]>("changes_list");
export const changeGet = (rel: string) => call<ChangeText>("change_get", { rel });
/** Writes one reviewed change with the version check; returns the file's new version. */
export const changeAccept = (rel: string) => call<string>("change_accept", { rel });
export const changeReject = (rel: string) => call<void>("change_reject", { rel });

export async function chatSend(model: string, messages: Msg[], onText: (t: string) => void): Promise<StreamEnd> {
  const onEvent = new Channel<ChatEvent>();
  onEvent.onmessage = (ev) => { if (ev.kind === "delta") onText(ev.text); };
  return call<StreamEnd>("chat_send", { model, messages, onEvent });
}
export const chatStop = () => call<void>("chat_stop");
export const chatsList = () => call<ChatSummary[]>("chats_list");
export const chatLoad = (id: string) => call<Chat>("chat_load", { id });
export const chatSave = (chat: Chat) => call<Chat>("chat_save", { chat });
export const chatDelete = (id: string) => call<void>("chat_delete", { id });
export const chatsSearch = (query: string) => call<ChatHit[]>("chats_search", { query });

// ---- Missions (phase I4, docs/features/missions.md). Every field is cleaned in Rust; still rendered as text only.

export interface Switches {
  autopilot_available: boolean; improve_available: boolean; disabled: boolean;
  autopilot: boolean; auto_improve: boolean; resource_scope: string;
}
export interface SyncView {
  state: string; version: number; delta_state: string; delta_version: number;
  delta_added: number; delta_modified: number; delta_deleted: number; delta_conflicts: string[];
}
export interface ProjectState {
  gateway_name: string; project_id: string | null; project_name: string;
  switches: Switches | null; sync: SyncView | null; note: string;
  /** The gateway has no P2P-AI projects API (a plain model endpoint such as Ollama): it can chat but not host missions. */
  no_projects: boolean;
}
export interface ProjectView { id: string; name: string; role: string; version: number; file_count: number }
export interface Skipped { rel: string; reason: string }
export interface PushReport { uploaded: number; deleted: number; unchanged: number; skipped: Skipped[]; version: number; copy: string }
export interface PullReport { proposed: number; same: number; only_local: number; skipped: Skipped[] }
export interface TaskView { title: string; state: string }
export interface CheckView { id: string; passed: boolean; exit_code: number; error: string }
export interface MissionView {
  id: string; kind: string; project_id: string; model: string; goal: string; state: string; reason: string;
  cycle: number; max_cycles: number; tokens: number; run_id: string; waiting_approval: boolean;
  checks_passed: number; checks_total: number; tasks_done: number; tasks_total: number; proposals_created: number;
  updated_at: number; tasks: TaskView[]; checks: CheckView[];
}
export interface ApprovalView { id: string; tool: string; access: string; arguments: string; arguments_sha256: string; expires_at: number }
export interface ProposalView {
  id: string; title: string; kind: string; rationale: string; effort: string; steps: string[]; sources: string[]; state: string;
}
/** An acceptance check: a test framework, or a command split into words (no shell). */
export interface CheckInput { id: string; kind: "tests" | "command"; framework?: string; argv?: string[] }

export const missionProject = () => call<ProjectState>("mission_project");
export const projectsList = () => call<ProjectView[]>("projects_list");
export const projectCreate = (name: string) => call<ProjectView>("project_create", { name });
export const projectLink = (projectId: string | null, projectName: string) => call<void>("project_link", { projectId, projectName });
/** Sends the open folder (minus secrets) to the linked project and asks for a node copy. */
export const projectPush = () => call<PushReport>("project_push");
/** Collects the node copy's changes into the gateway project. */
export const projectCollect = () => call<SyncView>("project_collect");
/** Brings changed files back into "Changes to review" (nothing is written). */
export const projectPull = () => call<PullReport>("project_pull");
export const switchesSet = (autopilot: boolean, autoImprove: boolean) => call<Switches>("switches_set", { autopilot, autoImprove });
export const missionsList = () => call<MissionView[]>("missions_list");
export const missionGet = (id: string) => call<MissionView>("mission_get", { id });
export const missionStart = (kind: "autopilot" | "improve", goal: string, model: string, checks: CheckInput[]) =>
  call<MissionView>("mission_start", { kind, goal, model, checks });
export const missionControl = (id: string, action: "pause" | "resume" | "stop") => call<void>("mission_control", { id, action });
export const approvalsList = (runId: string) => call<ApprovalView[]>("approvals_list", { runId });
export const approvalDecide = (runId: string, approvalId: string, decision: "approve" | "reject", argumentsSha256: string) =>
  call<void>("approval_decide", { runId, approvalId, decision, argumentsSha256 });
export const proposalsList = () => call<ProposalView[]>("proposals_list");
export const proposalAct = (id: string, action: "accept" | "dismiss" | "snooze") => call<void>("proposal_act", { id, action });

// ---- Phase I5: source control (src-tauri/src/scm.rs, docs/features/source-control.md)

/** One changed file: git's status letters for the staged (`index`) and unstaged (`work`) side; "." = none. */
export interface ScmFile { rel: string; index: string; work: string; conflict: boolean; untracked: boolean; actionable: boolean }
export interface ScmStatus { branch: string; initial: boolean; ahead: number; behind: number; files: ScmFile[]; truncated: boolean }
/** `available` is false (with a fixed `reason`) when the folder is not a repository, git is missing or refused. */
export interface ScmState { available: boolean; reason: string; status: ScmStatus | null }
export interface ScmDiff { rel: string; before: string; after: string; deleted: boolean }
export interface ScmCommit { id: string; subject: string; author: string; time: number }

export const scmStatus = () => call<ScmState>("scm_status");
export const scmDiff = (rel: string) => call<ScmDiff>("scm_diff", { rel });
/** Stages one file, or every change with null. */
export const scmStage = (rel: string | null) => call<void>("scm_stage", { rel });
export const scmUnstage = (rel: string | null) => call<void>("scm_unstage", { rel });
/** Commits what is staged (hooks off); resolves to the short commit id. */
export const scmCommit = (message: string) => call<string>("scm_commit", { message });
export const scmLog = () => call<ScmCommit[]>("scm_log");

// ---- Agent commits: git_commit mode "local" (src-tauri/src/agent_commits.rs, docs/features/agent-commits.md)

/** An agent's checkpoint that waits for the person's commit in the open folder. */
export interface AgentCheckpoint { id: string; version: number; subject: string; message: string; created_at: number; local: string }
/** The checked diff (plain text, shown read-only) and the files it changes. */
export interface AgentCommitDiff { diff: string; files: string[] }

export const agentCommitsPending = () => call<AgentCheckpoint[]>("agent_commits_pending");
export const agentCommitDiff = (cid: string) => call<AgentCommitDiff>("agent_commit_diff", { cid });
/** Applies and commits it with the person's own git identity; resolves to the new commit id. Only from the Commit button. */
export const agentCommitApply = (cid: string) => call<string>("agent_commit_apply", { cid });
export const agentCommitReject = (cid: string) => call<void>("agent_commit_reject", { cid });

// ---- Phase I6a: extensions from Open VSX (src-tauri/src/ext.rs, docs/features/extensions.md)

/** One Open VSX search result. Plain text only: render as text, never HTML. */
export interface ExtSummary { namespace: string; name: string; version: string; display_name: string; description: string; downloads: number }
/** An installed extension. `kind` is what it needs to run: "declarative", "web" (worker host) or "node" (I6c Node host).
 * `network`: the person allowed this web extension to reach public https sites (phase I6b). */
export interface ExtInstalled {
  id: string; namespace: string; name: string; version: string; display_name: string; description: string;
  kind: "declarative" | "web" | "node"; enabled: boolean; installed_at: number; sha256: string; verified_publisher: boolean; folder: string;
  network?: boolean;
}

export const extSearch = (query: string, offset = 0) => call<ExtSummary[]>("ext_search", { query, offset });
/** Downloads, verifies (SHA-256 and Open VSX signature) and installs; only call on the person's click. */
export const extInstall = (namespace: string, name: string, version = "latest") => call<ExtInstalled>("ext_install", { namespace, name, version });
export const extList = () => call<ExtInstalled[]>("ext_list");
export const extSetEnabled = (id: string, enabled: boolean) => call<ExtInstalled>("ext_set_enabled", { id, enabled });
export const extUninstall = (id: string) => call<void>("ext_uninstall", { id });
export const extManifest = (id: string) => call<Record<string, unknown>>("ext_manifest", { id });
/** One file of an enabled extension, as bytes. */
export const extFile = (id: string, path: string) => call<ArrayBuffer>("ext_file", { id, path });
/** Allows or stops internet for a web extension; only call on the person's click (phase I6b). */
export const extSetNetwork = (id: string, allowed: boolean) => call<ExtInstalled>("ext_set_network", { id, allowed });
/** A web extension's https request, body as base64; the core checks it (public https only) and makes it. */
export interface ExtNetCall { method: string; url: string; headers: [string, string][]; body_b64: string }
export interface ExtNetAnswer { status: number; status_text: string; headers: [string, string][]; url: string; body_b64: string }
export const extFetch = (id: string, request: ExtNetCall) => call<ExtNetAnswer>("ext_fetch", { id, request });

/** Phase I6c: the Node extension host for the open folder (node_host_status). */
export interface NodeHostStatus {
  /** A server is pinned for this OS and CPU; otherwise Node extensions can't run on this computer yet. */
  pinned: boolean;
  /** The pinned server is already downloaded. */
  installed: boolean;
  /** The open folder is trusted for Node extensions (false with no folder open). */
  trusted: boolean;
  /** The port on 127.0.0.1 while the server runs. */
  port: number | null;
  /** The VS Code version the pinned server is built from ("" when nothing is pinned); see lib/vscodeVersion.ts. */
  vscode: string;
  /** Phase I6d: the optional Electron compatibility host (off by default). */
  electron: NodeHostElectron;
}
/** Phase I6d: the Electron compatibility host's state (node_host_status). */
export interface NodeHostElectron {
  /** Electron is pinned for this computer. */
  available: boolean;
  /** The pinned Electron is downloaded. */
  installed: boolean;
  /** The person turned compatibility on. */
  enabled: boolean;
  /** The running host uses Electron. */
  running: boolean;
  /** The pinned Electron version. */
  version: string;
  /** Enabled Node extensions with native modules built for one runtime (shown as text only). */
  needed_by: string[];
}
/** Where the editor connects to the running server. SECURITY: the token stays in memory only (CWE-522). */
export interface NodeHostConnection {
  port: number;
  token: string;
  folder: string;
  /** The server's VS Code commit; the editor announces it so the server accepts the connection. */
  commit: string;
  /** The VS Code version the server is built from; the editor connects only when its own matches. */
  vscode: string;
}
export const nodeHostStatus = () => call<NodeHostStatus>("node_host_status");
/** Trusts or stops trusting the open folder. Only from the person's click. */
export const nodeHostTrust = (trusted: boolean) => call<null>("node_host_trust", { trusted });
/** Starts (or reuses) the server; `allowDownload` only after the person agreed to the download. */
export const nodeHostStart = (allowDownload: boolean) => call<NodeHostConnection>("node_host_start", { allowDownload });
export const nodeHostStop = () => call<null>("node_host_stop");
/** Phase I6d: turns the Electron compatibility host on or off (stops a running host). Only from the person's click. */
export const nodeHostElectron = (enabled: boolean) => call<null>("node_host_electron", { enabled });
