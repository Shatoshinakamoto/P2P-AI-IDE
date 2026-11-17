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
  kind: "local" | "manual";
  /** Plain http on loopback: shown as INSECURE - DEV ONLY. */
  dev: boolean;
  pinned: boolean;
  has_key: boolean;
}
export interface GatewayList { gateways: GatewayView[]; active: string | null }
export interface GatewayCheck { latency_ms: number; models: string[] }
export interface Msg { role: "system" | "user" | "assistant"; content: string }
export interface StreamEnd { finish_reason: string | null; chars: number }
export interface ChatMessage { role: "user" | "assistant" | "error"; content: string; at: number }
export interface Chat { id: string; title: string; gateway: string; model: string; created: number; updated: number; messages: ChatMessage[] }
export interface ChatSummary { id: string; title: string; updated: number; count: number }
export interface ChatHit { id: string; title: string; preview: string }
type ChatEvent = { kind: "delta"; text: string };

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

/**
 * Sends the conversation to the active gateway. `onText` gets each piece of the answer as it streams in.
 * SECURITY: answer text is untrusted; callers render it as text only (CWE-79).
 */
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
