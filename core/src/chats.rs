//! chats -- chat history saved on this computer, one file per chat, grouped by workspace (phase I2).
//!
//! Purpose: the chat panel keeps every conversation so people can come back to it, search it and delete it, like
//! the history in Hermes or OpenClaw. Chats are stored in the app data folder under `chats/<scope>/<id>.json`,
//! where `scope` is a hash of the open folder's path (or `none` without a folder), so each workspace has its own
//! history and the folder path itself is not written into the history folder names.
//!
//! Inputs and trust boundaries: chats come from the webview (untrusted): every field is checked and capped before
//! saving, and re-checked on load. Message text is stored and returned as text only.
//!
//! Security controls:
//!   * Ids are 16 hex characters and scopes 16 hex or `none`, so they can only name files in this folder
//!     (CWE-22, CWE-73).
//!   * Files are owner-only (0600, folders 0700 on Unix) via [`crate::store`], since chats can hold private code
//!     (CWE-276, CWE-200). Nothing in a chat is a secret by design: keys never enter chat text from the app.
//!   * Caps: 500 chats per workspace, 400 messages and 4 MiB per chat, 120-character titles, search over at most
//!     500 chats with a 256-byte plain-substring query, 200 hits (CWE-400, CWE-1333: no regular expressions).
//!   * Errors are fixed strings (CWE-209).
//!
//! Dependencies: serde/serde_json, sha2 (the scope hash), [`crate::store`].

use crate::store::{self, StoreError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Most chats kept per workspace; saving more is refused until some are deleted.
pub const MAX_CHATS: usize = 500;
/// Most messages in one saved chat.
pub const MAX_CHAT_MESSAGES: usize = 400;
/// Largest saved chat file.
pub const MAX_CHAT_BYTES: usize = 4 << 20;
/// Most search hits returned.
const MAX_HITS: usize = 200;

/// One saved message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    /// `user`, `assistant` or `error` (an error line shown in the chat, never sent to a model).
    pub role: String,
    pub content: String,
    /// Seconds since 1970.
    pub at: i64,
}

/// One saved chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chat {
    pub id: String,
    pub title: String,
    /// Display name of the gateway used last (not its URL or key).
    pub gateway: String,
    pub model: String,
    pub created: i64,
    pub updated: i64,
    pub messages: Vec<ChatMessage>,
}

/// A line in the history list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated: i64,
    pub count: usize,
}

/// A search hit: the chat and a short preview around the match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChatHit {
    pub id: String,
    pub title: String,
    pub preview: String,
}

/// Fixed errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatError {
    BadChat(&'static str),
    NotFound,
    Full,
    Store(StoreError),
}

impl ChatError {
    pub fn message(&self) -> &'static str {
        match self {
            ChatError::BadChat(m) => m,
            ChatError::NotFound => "that chat is not in the history",
            ChatError::Full => "the history holds 500 chats for this folder; delete some first",
            ChatError::Store(e) => e.message(),
        }
    }
}

/// The history scope for an open folder: 16 hex of SHA-256 of its canonical path, or `none`.
pub fn scope_for(root: Option<&Path>) -> String {
    match root {
        Some(p) => {
            let h = Sha256::digest(p.to_string_lossy().as_bytes());
            h.iter().take(8).map(|b| format!("{b:02x}")).collect()
        }
        None => "none".into(),
    }
}

fn scope_ok(s: &str) -> bool {
    s == "none" || store::id_ok(s)
}

/// Makes a title from the first user message: first line, at most 120 characters.
pub fn title_from(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("New chat");
    let t: String = first.chars().filter(|c| !c.is_control()).take(120).collect();
    if t.is_empty() {
        "New chat".into()
    } else {
        t
    }
}

/// Checks a chat from the webview (or from disk). SECURITY: allow-listed roles, bounded sizes, safe id (CWE-20).
pub fn check_chat(c: &Chat) -> Result<(), ChatError> {
    if !store::id_ok(&c.id) {
        return Err(ChatError::BadChat("the chat id is not valid"));
    }
    if c.title.chars().count() > 120 || c.title.chars().any(char::is_control) {
        return Err(ChatError::BadChat("the chat title must be one line of up to 120 characters"));
    }
    if c.gateway.chars().count() > 64 || c.model.len() > 200 {
        return Err(ChatError::BadChat("the gateway or model name is too long"));
    }
    if c.messages.len() > MAX_CHAT_MESSAGES {
        return Err(ChatError::BadChat("a chat can hold up to 400 messages; start a new chat"));
    }
    if c.messages.iter().any(|m| !matches!(m.role.as_str(), "user" | "assistant" | "error")) {
        return Err(ChatError::BadChat("a message has an unknown role"));
    }
    Ok(())
}

/// Chat history for one workspace scope.
pub struct ChatStore {
    dir: PathBuf,
}

impl ChatStore {
    /// `data_dir` is the app data folder (from the shell); `scope` from [`scope_for`].
    pub fn new(data_dir: &Path, scope: &str) -> Result<ChatStore, ChatError> {
        if !scope_ok(scope) {
            return Err(ChatError::BadChat("bad history scope"));
        }
        Ok(ChatStore { dir: data_dir.join("chats").join(scope) })
    }

    fn path(&self, id: &str) -> Result<PathBuf, ChatError> {
        if !store::id_ok(id) {
            return Err(ChatError::NotFound);
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    /// Ids of saved chats (at most [`MAX_CHATS`] + 1 are looked at).
    fn ids(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return vec![];
        };
        rd.filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(str::to_string))
            .filter(|id| store::id_ok(id))
            .take(MAX_CHATS + 1)
            .collect()
    }

    /// Loads one chat.
    pub fn load(&self, id: &str) -> Result<Chat, ChatError> {
        let bytes = store::read_capped(&self.path(id)?, MAX_CHAT_BYTES as u64).map_err(|e| match e {
            StoreError::Missing => ChatError::NotFound,
            e => ChatError::Store(e),
        })?;
        let c: Chat = serde_json::from_slice(&bytes).map_err(|_| ChatError::Store(StoreError::Bad))?;
        check_chat(&c)?;
        if c.id != id {
            return Err(ChatError::Store(StoreError::Bad));
        }
        Ok(c)
    }

    /// Saves a chat (new or replacing the one with the same id). Returns it as saved (title filled in, times set).
    pub fn save(&self, mut c: Chat) -> Result<Chat, ChatError> {
        check_chat(&c)?;
        let path = self.path(&c.id)?;
        let exists = path.exists();
        if !exists && self.ids().len() >= MAX_CHATS {
            return Err(ChatError::Full);
        }
        if c.title.trim().is_empty() {
            c.title = c
                .messages
                .iter()
                .find(|m| m.role == "user")
                .map(|m| title_from(&m.content))
                .unwrap_or_else(|| "New chat".into());
        }
        let now = store::now_s();
        if c.created <= 0 {
            c.created = now;
        }
        c.updated = now;
        let bytes = serde_json::to_vec(&c).map_err(|_| ChatError::Store(StoreError::Bad))?;
        if bytes.len() > MAX_CHAT_BYTES {
            return Err(ChatError::BadChat("this chat is larger than 4 MiB; start a new chat"));
        }
        store::write_atomic(&path, &bytes).map_err(ChatError::Store)?;
        Ok(c)
    }

    /// Deletes a chat.
    pub fn delete(&self, id: &str) -> Result<(), ChatError> {
        let p = self.path(id)?;
        if !p.exists() {
            return Err(ChatError::NotFound);
        }
        store::remove(&p).map_err(ChatError::Store)
    }

    /// The history list, newest first. Damaged files are skipped, not shown.
    pub fn list(&self) -> Vec<ChatSummary> {
        let mut out: Vec<ChatSummary> = self
            .ids()
            .iter()
            .filter_map(|id| self.load(id).ok())
            .map(|c| ChatSummary { id: c.id, title: c.title, updated: c.updated, count: c.messages.len() })
            .collect();
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
        out
    }

    /// Plain, case-insensitive substring search over titles and message text.
    /// SECURITY: no regular expressions (CWE-1333); query capped at 256 bytes; hits capped (CWE-400).
    pub fn search(&self, query: &str) -> Result<Vec<ChatHit>, ChatError> {
        let q = query.trim();
        if q.is_empty() || q.len() > 256 || q.chars().any(char::is_control) {
            return Err(ChatError::BadChat("search text must be 1 to 256 characters on one line"));
        }
        let ql = q.to_lowercase();
        let mut hits = Vec::new();
        for s in self.list() {
            let Ok(c) = self.load(&s.id) else { continue };
            let found = if c.title.to_lowercase().contains(&ql) {
                Some(c.title.clone())
            } else {
                c.messages.iter().find_map(|m| preview(&m.content, &ql))
            };
            if let Some(p) = found {
                hits.push(ChatHit { id: c.id, title: c.title, preview: p });
                if hits.len() >= MAX_HITS {
                    break;
                }
            }
        }
        Ok(hits)
    }
}

/// About 100 characters around the first match, on one line.
fn preview(text: &str, ql: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(ql)?;
    // Work in chars so a multi-byte character is never cut (lowercasing can change byte lengths, so map by ratio).
    let chars: Vec<char> = text.chars().collect();
    let pos = lower[..at].chars().count().min(chars.len());
    let start = pos.saturating_sub(40);
    let end = (pos + 60).min(chars.len());
    let s: String = chars[start..end].iter().map(|c| if c.is_control() { ' ' } else { *c }).collect();
    Some(s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-chats-{tag}-{}", store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn chat(text: &str) -> Chat {
        Chat {
            id: store::new_id(),
            title: String::new(),
            gateway: "Home".into(),
            model: "tiny".into(),
            created: 0,
            updated: 0,
            messages: vec![
                ChatMessage { role: "user".into(), content: text.into(), at: 1 },
                ChatMessage { role: "assistant".into(), content: "Use a <script> tag? No: text only.".into(), at: 2 },
            ],
        }
    }

    #[test]
    fn save_list_load_search_delete() {
        let d = dir("crud");
        let s = ChatStore::new(&d, &scope_for(Some(Path::new("/work/shop-api")))).unwrap();
        let a = s.save(chat("How do I add a timeout?\nmore")).unwrap();
        assert_eq!(a.title, "How do I add a timeout?");
        assert!(a.created > 0 && a.updated >= a.created);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let b = s.save(chat("Explain orders.py")).unwrap();
        let list = s.list();
        assert_eq!(list.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec![b.id.as_str(), a.id.as_str()]);
        assert_eq!(s.load(&a.id).unwrap(), a);
        let hits = s.search("TIMEOUT").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a.id);
        assert!(s.search("script").unwrap().len() == 2);
        assert!(s.search("").is_err());
        s.delete(&a.id).unwrap();
        assert_eq!(s.load(&a.id), Err(ChatError::NotFound));
        assert_eq!(s.delete(&a.id), Err(ChatError::NotFound));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn scopes_keep_workspaces_apart_and_hide_the_path() {
        let d = dir("scope");
        let one = scope_for(Some(Path::new("/work/one")));
        let two = scope_for(Some(Path::new("/work/two")));
        assert_ne!(one, two);
        assert!(!one.contains("work"));
        assert_eq!(scope_for(None), "none");
        let s1 = ChatStore::new(&d, &one).unwrap();
        let s2 = ChatStore::new(&d, &two).unwrap();
        s1.save(chat("only in one")).unwrap();
        assert_eq!(s1.list().len(), 1);
        assert!(s2.list().is_empty());
        assert!(ChatStore::new(&d, "../x").is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn bad_chats_and_ids_are_refused() {
        let d = dir("bad");
        let s = ChatStore::new(&d, "none").unwrap();
        let mut c = chat("x");
        c.id = "../../etc/passwd".into();
        assert!(s.save(c).is_err());
        let mut c = chat("x");
        c.messages[0].role = "system".into();
        assert!(s.save(c).is_err());
        let mut c = chat("x");
        c.title = "a\nb".into();
        assert!(s.save(c).is_err());
        assert_eq!(s.load("../../secret"), Err(ChatError::NotFound));
        // A file whose inner id does not match its name is treated as damaged.
        let good = s.save(chat("x")).unwrap();
        let other = store::new_id();
        std::fs::copy(
            d.join("chats/none").join(format!("{}.json", good.id)),
            d.join("chats/none").join(format!("{other}.json")),
        )
        .unwrap();
        assert_eq!(s.load(&other), Err(ChatError::Store(StoreError::Bad)));
        assert_eq!(s.list().len(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn titles_and_previews_are_one_line_and_bounded() {
        assert_eq!(title_from("\n\n  hello\nworld"), "hello");
        assert_eq!(title_from(""), "New chat");
        assert_eq!(title_from(&"é".repeat(300)).chars().count(), 120);
        let p = preview(&format!("{}needle{}", "a\n".repeat(100), "b".repeat(200)), "needle").unwrap();
        assert!(p.contains("needle") && !p.contains('\n') && p.chars().count() <= 100);
    }
}
