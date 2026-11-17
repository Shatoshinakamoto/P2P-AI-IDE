//! p2p-ide-core -- the P2P-AI IDE's Rust core (docs/features/editor-shell.md).
//!
//! Purpose: every operation on the user's files goes through this crate, so the Tauri shell stays a thin layer of
//! typed commands and the same checks later bind the local agent tools (phase I3).
//! Trust boundary: callers pass webview input straight in; the crate checks it (see workspace.rs for the controls).
//! Dependencies: serde, sha2 (see Cargo.toml).

pub mod workspace;

pub use workspace::{Entry, EntryKind, FileText, Listing, Match, SearchResult, Workspace, WsError};
