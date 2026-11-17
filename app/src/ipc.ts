/**
 * ipc.ts -- typed wrappers for the six Tauri commands in src-tauri/src/main.rs.
 *
 * Purpose: one place where the webview talks to the Rust core, with the record shapes it returns.
 * Trust boundary: the webview is untrusted to the core; the core checks every argument (paths, sizes, queries).
 * Errors arrive as short fixed strings safe to show (CWE-209). Nothing here reads files or the network itself.
 */
import { invoke } from "@tauri-apps/api/core";

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
