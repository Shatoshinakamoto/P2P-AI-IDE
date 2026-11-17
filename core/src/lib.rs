//! p2p-ide-core -- the P2P-AI IDE's Rust core (docs/features/editor-shell.md).
//!
//! Purpose: every operation on the user's files goes through this crate, so the Tauri shell stays a thin layer of
//! typed commands and the same checks later bind the local agent tools (phase I3).
//! Trust boundary: callers pass webview input straight in; the crate checks it (see workspace.rs for the controls).
//! Phase I2 adds the network side: saved gateways ([`gateways`]), the HTTPS client for models and streamed chat
//! ([`client`]), chat history ([`chats`]) and the small-file store they share ([`store`]).
//! Dependencies: serde, serde_json, sha2, ureq (see Cargo.toml).

pub mod chats;
pub mod client;
pub mod discovery;
pub mod gateways;
pub mod store;
pub mod workspace;

pub use workspace::{Entry, EntryKind, FileText, Listing, Match, SearchResult, Workspace, WsError};
