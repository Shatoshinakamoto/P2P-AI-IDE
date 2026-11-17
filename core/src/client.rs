//! client -- the IDE's HTTPS client for one gateway: model list and streamed chat (phase I2).
//!
//! Purpose: the chat panel sends a conversation to the active gateway's OpenAI-style API and shows the answer as it
//! streams in. The webview never talks to a gateway itself; it calls Tauri commands, and this module makes the
//! requests from Rust with the key from the OS keychain.
//!
//! | Call | Request |
//! |---|---|
//! | [`Client::models`] | `GET /v1/models` -> `{data: [{id}]}` |
//! | [`Client::chat_stream`] | `POST /v1/chat/completions` with `stream: true` -> server-sent events of `chat.completion.chunk` |
//!
//! Inputs and trust boundaries: the conversation and model name come from the webview (untrusted, checked here);
//! every gateway answer is untrusted: parsed into fixed shapes with size caps and handed to the webview as plain
//! text, which renders it as text, never HTML (CWE-79). Answers are never run or treated as instructions.
//!
//! Security controls:
//!   * TLS on for every non-loopback gateway (rustls, `https_only`); a pinned CA, when saved, is the only trust root
//!     for that gateway, otherwise the Mozilla roots in webpki-roots (CWE-295, NIST SC-8, SC-13).
//!   * No redirects, so a gateway cannot bounce the key to another host (CWE-601, CWE-522); no proxy from the
//!     environment, so the key only goes to the gateway's own address.
//!   * The key is sent only as `Authorization: Bearer`, never logged, never in error text (CWE-532).
//!   * Caps: request 2 MiB, 200 messages, answer stream 8 MiB, one event line 1 MiB, model list 1 MiB / 1000 ids,
//!     10 s connect, 15 s for the model list, 10 min for a chat turn; the person can stop a turn (CWE-400, CWE-770).
//!   * Errors are fixed messages by kind with the HTTP status at most (CWE-209).
//!
//! Dependencies: ureq 3 (rustls + ring, same as the P2P-AI desktop app), serde_json.

use crate::discovery::PublicOnly;
use crate::gateways::{Gateway, Kind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Largest request body sent.
pub const MAX_REQUEST_BYTES: usize = 2 << 20;
/// Most messages in one request.
pub const MAX_MESSAGES: usize = 200;
/// Largest single message.
pub const MAX_MESSAGE_BYTES: usize = 256 << 10;
/// Largest streamed answer read.
pub const MAX_STREAM_BYTES: u64 = 8 << 20;
/// Longest single event line.
const MAX_LINE_BYTES: usize = 1 << 20;
/// Largest model list read, and most ids kept.
const MAX_MODELS_BYTES: u64 = 1 << 20;
const MAX_MODELS: usize = 1000;

/// Fixed error kinds. `message()` never holds a URL, key or answer text (CWE-209).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    NoKey,
    BadRequest(&'static str),
    Connect,
    Tls,
    Timeout,
    Auth,
    RateLimited,
    NotOffered,
    Status(u16),
    Upstream,
    BadAnswer,
    TooLarge,
    Stopped,
    /// A known gateway refusal (phase I4), with a fixed message chosen from the gateway's error code.
    Gateway(&'static str),
}

impl NetError {
    pub fn message(&self) -> String {
        match self {
            NetError::NoKey => "this gateway has no key yet; add one in the Gateways view".into(),
            NetError::BadRequest(m) => (*m).into(),
            NetError::Connect => "could not reach the gateway".into(),
            NetError::Tls => "the gateway's certificate is not trusted (check its CA file)".into(),
            NetError::Timeout => "the gateway took too long to answer".into(),
            NetError::Auth => "the gateway refused the key".into(),
            NetError::RateLimited => "the gateway is limiting requests; try again shortly".into(),
            NetError::NotOffered => "this gateway does not offer that".into(),
            NetError::Status(s) => format!("the gateway answered with an error (HTTP {s})"),
            NetError::Upstream => "the gateway reported an error while answering".into(),
            NetError::BadAnswer => "the gateway's answer could not be read".into(),
            NetError::TooLarge => "the gateway's answer was too large".into(),
            NetError::Stopped => "stopped".into(),
            NetError::Gateway(m) => (*m).into(),
        }
    }
}

/// One chat message. Roles are an allow-list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

/// Checks a model id: 1-200 visible characters (gateways use names like `llama3.1:8b`).
pub fn check_model(m: &str) -> Result<(), NetError> {
    if m.is_empty() || m.len() > 200 || !m.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(NetError::BadRequest("pick a model from the list"));
    }
    Ok(())
}

/// Checks a conversation before it is sent (CWE-20, CWE-400).
pub fn check_messages(msgs: &[Msg]) -> Result<(), NetError> {
    if msgs.is_empty() || msgs.len() > MAX_MESSAGES {
        return Err(NetError::BadRequest("a chat can hold 1 to 200 messages; start a new chat"));
    }
    let mut total = 0;
    for m in msgs {
        if !matches!(m.role.as_str(), "system" | "user" | "assistant") {
            return Err(NetError::BadRequest("a message has an unknown role"));
        }
        if m.content.len() > MAX_MESSAGE_BYTES {
            return Err(NetError::BadRequest("a message is larger than 256 KiB"));
        }
        total += m.content.len();
    }
    if total > MAX_REQUEST_BYTES - 4096 {
        return Err(NetError::BadRequest("the chat is larger than 2 MiB; start a new chat"));
    }
    Ok(())
}

/// How a streamed turn ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StreamEnd {
    /// `stop`, `length`, ... as the gateway said, or `null`.
    pub finish_reason: Option<String>,
    /// Bytes of answer text received.
    pub chars: usize,
}

/// One streamed piece of a tool call. OpenAI streams a call as several pieces with the same `index`: the first
/// carries `id` and `name`, later ones add to `arguments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPiece {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub args: String,
}

/// A complete tool call the model asked for. Untrusted: the agent checks the name and arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments as the JSON text the model wrote.
    pub arguments: String,
}

/// Most tool calls accepted in one answer, and the largest arguments text of one call (CWE-400).
pub const MAX_TOOL_CALLS: usize = 8;
pub const MAX_TOOL_ARGS_BYTES: usize = 1 << 20;

/// One parsed server-sent event line.
#[derive(Debug, PartialEq, Eq)]
pub enum SseItem {
    /// Answer text to append.
    Delta(String),
    /// The model's finish reason.
    Finish(String),
    /// Pieces of tool calls (phase I3): `delta.tool_calls[]` as `(index, id, name, arguments fragment)`.
    Tools(Vec<ToolPiece>),
    /// `data: [DONE]`.
    Done,
    /// The gateway sent an error object.
    Error,
    /// Comments, keep-alives, role-only deltas, usage chunks.
    Skip,
}

/// Parses one SSE line from `/v1/chat/completions`. Pure, so it is tested without a network.
/// SECURITY: unknown shapes are skipped or refused, never executed; text stays text (CWE-20).
pub fn parse_sse_line(line: &str) -> Result<SseItem, NetError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(SseItem::Skip); // blank lines, `: keep-alive`, `event:` lines
    };
    let data = data.trim_start();
    if data == "[DONE]" {
        return Ok(SseItem::Done);
    }
    let v: Value = serde_json::from_str(data).map_err(|_| NetError::BadAnswer)?;
    if v.get("error").is_some() {
        return Ok(SseItem::Error);
    }
    let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
        return Ok(SseItem::Skip);
    };
    if let Some(tc) = choice.get("delta").and_then(|d| d.get("tool_calls")).and_then(Value::as_array) {
        if !tc.is_empty() {
            let mut pieces = Vec::new();
            for (pos, c) in tc.iter().enumerate() {
                let index = c.get("index").and_then(Value::as_u64).map(|i| i as usize).unwrap_or(pos);
                let f = c.get("function");
                let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("").to_string();
                // Ollama sends arguments as an object; OpenAI as a JSON string. Both end up as JSON text.
                let args = match f.and_then(|f| f.get("arguments")) {
                    Some(Value::String(a)) => a.clone(),
                    Some(v @ Value::Object(_)) => v.to_string(),
                    _ => String::new(),
                };
                pieces.push(ToolPiece {
                    index,
                    id: text(c.get("id")),
                    name: text(f.and_then(|f| f.get("name"))),
                    args,
                });
            }
            return Ok(SseItem::Tools(pieces));
        }
    }
    if let Some(t) = choice.get("delta").and_then(|d| d.get("content")).and_then(Value::as_str) {
        if !t.is_empty() {
            return Ok(SseItem::Delta(t.to_string()));
        }
    }
    if let Some(f) = choice.get("finish_reason").and_then(Value::as_str) {
        return Ok(SseItem::Finish(f.chars().take(32).collect()));
    }
    Ok(SseItem::Skip)
}

/// A client bound to one gateway and its key.
pub struct Client {
    pub(crate) base: String,
    key: String,
    pub(crate) short: ureq::Agent,
    pub(crate) long: ureq::Agent,
}

impl std::fmt::Debug for Client {
    // SECURITY: never print the key (CWE-532).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("base", &self.base).finish_non_exhaustive()
    }
}

impl Client {
    /// Builds a client. `key` comes from the keychain; a gateway without a key cannot be used.
    pub fn new(gw: &Gateway, key: Option<String>) -> Result<Client, NetError> {
        let key = key.ok_or(NetError::NoKey)?;
        let roots = match &gw.ca_pem {
            Some(pem) => {
                let certs = ureq::tls::parse_pem(pem.as_bytes())
                    .filter_map(|i| match i {
                        Ok(ureq::tls::PemItem::Certificate(c)) => Some(c.to_owned()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if certs.is_empty() {
                    return Err(NetError::Tls);
                }
                // SECURITY: the pinned CA is the only root for this gateway (CWE-295).
                ureq::tls::RootCerts::Specific(Arc::new(certs))
            }
            None => ureq::tls::RootCerts::WebPki,
        };
        let tls = ureq::tls::TlsConfig::builder().root_certs(roots).build();
        let make = |total: Duration| -> ureq::Agent {
            let config = ureq::Agent::config_builder()
                .tls_config(tls.clone())
                // SECURITY: no proxy from the environment and no redirects, so the key reaches only this host.
                .proxy(None)
                .max_redirects(0)
                // SECURITY: https only, except the loopback dev gateway the URL check allowed (NIST SC-8).
                .https_only(!gw.dev())
                .http_status_as_error(false)
                .timeout_connect(Some(Duration::from_secs(10)))
                .timeout_global(Some(total))
                .user_agent("p2p-ai-ide/0.1")
                .build();
            if gw.kind == Kind::Public {
                // SECURITY: a gateway from the public list may only be reached at a public address, even if its
                // name later resolves elsewhere (DNS rebinding, CWE-918); the key never goes to a private host.
                ureq::Agent::with_parts(config, ureq::unversioned::transport::DefaultConnector::default(), PublicOnly)
            } else {
                config.into()
            }
        };
        Ok(Client {
            base: gw.url.clone(),
            key,
            short: make(Duration::from_secs(15)),
            long: make(Duration::from_secs(600)),
        })
    }

    pub(crate) fn auth(&self) -> String {
        format!("Bearer {}", self.key)
    }

    /// Lists the gateway's model ids and the round-trip time in milliseconds.
    pub fn models(&self) -> Result<(Vec<String>, u64), NetError> {
        let t0 = Instant::now();
        let resp = self
            .short
            .get(format!("{}/v1/models", self.base))
            .header("Authorization", self.auth())
            .header("Accept", "application/json")
            .call()
            .map_err(map_ureq)?;
        let ms = t0.elapsed().as_millis() as u64;
        check_status(resp.status().as_u16())?;
        let mut body = Vec::new();
        resp.into_body()
            .into_reader()
            .take(MAX_MODELS_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|_| NetError::BadAnswer)?;
        if body.len() as u64 > MAX_MODELS_BYTES {
            return Err(NetError::TooLarge);
        }
        Ok((parse_models(&body)?, ms))
    }

    /// Sends a conversation and calls `on_delta` with each piece of answer text as it arrives. `on_delta` returns
    /// `false` to stop (the person pressed Stop); the turn then ends with [`NetError::Stopped`].
    pub fn chat_stream(
        &self,
        model: &str,
        messages: &[Msg],
        on_delta: &mut dyn FnMut(&str) -> bool,
    ) -> Result<StreamEnd, NetError> {
        check_model(model)?;
        check_messages(messages)?;
        let body = json!({"model": model, "messages": messages, "stream": true}).to_string();
        if body.len() > MAX_REQUEST_BYTES {
            return Err(NetError::BadRequest("the chat is larger than 2 MiB; start a new chat"));
        }
        let resp = self
            .long
            .post(format!("{}/v1/chat/completions", self.base))
            .header("Authorization", self.auth())
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .send(body.as_bytes())
            .map_err(map_ureq)?;
        check_status(resp.status().as_u16())?;
        let reader = resp.into_body().into_reader().take(MAX_STREAM_BYTES);
        read_stream(BufReader::new(reader), on_delta)
    }
}

impl Client {
    /// One step of an agent turn (phase I3): sends `messages` (already checked by the agent) with the tool
    /// definitions, streams answer text to `on_delta`, and returns the text end and any tool calls.
    ///
    /// Security: same transport controls as [`Client::chat_stream`]; the body is capped at 2 MiB; tool calls are
    /// returned as untrusted text for the agent to check.
    pub fn agent_step(
        &self,
        model: &str,
        messages: &[Value],
        tools: &Value,
        on_delta: &mut dyn FnMut(&str) -> bool,
    ) -> Result<(StreamEnd, Vec<ToolCall>), NetError> {
        check_model(model)?;
        let body = json!({"model": model, "messages": messages, "tools": tools, "stream": true}).to_string();
        if body.len() > MAX_REQUEST_BYTES {
            return Err(NetError::BadRequest("the chat and the files read are larger than 2 MiB; start a new chat"));
        }
        let resp = self
            .long
            .post(format!("{}/v1/chat/completions", self.base))
            .header("Authorization", self.auth())
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .send(body.as_bytes())
            .map_err(map_ureq)?;
        check_status(resp.status().as_u16())?;
        let reader = resp.into_body().into_reader().take(MAX_STREAM_BYTES);
        read_stream_calls(BufReader::new(reader), on_delta)
    }
}

/// Reads an SSE body line by line. Split out so tests can feed it bytes.
pub fn read_stream(r: impl BufRead, on_delta: &mut dyn FnMut(&str) -> bool) -> Result<StreamEnd, NetError> {
    read_stream_calls(r, on_delta).map(|(end, _)| end)
}

/// Adds tool-call pieces to the calls collected so far (by index), with caps.
fn add_pieces(calls: &mut Vec<ToolCall>, pieces: Vec<ToolPiece>) -> Result<(), NetError> {
    for p in pieces {
        if p.index >= MAX_TOOL_CALLS {
            return Err(NetError::TooLarge);
        }
        while calls.len() <= p.index {
            calls.push(ToolCall { id: String::new(), name: String::new(), arguments: String::new() });
        }
        let c = &mut calls[p.index];
        if c.id.is_empty() && !p.id.is_empty() {
            c.id = p.id.chars().take(64).collect();
        }
        if c.name.is_empty() && !p.name.is_empty() {
            c.name = p.name.chars().take(64).collect();
        }
        c.arguments.push_str(&p.args);
        // SECURITY: arguments are capped while they stream in (CWE-400).
        if c.arguments.len() > MAX_TOOL_ARGS_BYTES {
            return Err(NetError::TooLarge);
        }
    }
    Ok(())
}

/// Like [`read_stream`], and also returns the tool calls the model asked for (phase I3).
pub fn read_stream_calls(
    mut r: impl BufRead,
    on_delta: &mut dyn FnMut(&str) -> bool,
) -> Result<(StreamEnd, Vec<ToolCall>), NetError> {
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut end = StreamEnd { finish_reason: None, chars: 0 };
    let mut total: u64 = 0;
    let mut line = Vec::new();
    loop {
        line.clear();
        // SECURITY: one line is bounded too, so a stream without newlines cannot fill memory (CWE-400).
        let n = (&mut r).take(MAX_LINE_BYTES as u64 + 1).read_until(b'\n', &mut line).map_err(map_io)?;
        if n == 0 {
            // The stream ended. An answer without [DONE] is still kept, as some gateways close early.
            if total >= MAX_STREAM_BYTES {
                return Err(NetError::TooLarge);
            }
            return Ok((end, calls));
        }
        total += n as u64;
        if line.len() > MAX_LINE_BYTES {
            return Err(NetError::TooLarge);
        }
        let text = std::str::from_utf8(&line).map_err(|_| NetError::BadAnswer)?;
        match parse_sse_line(text)? {
            SseItem::Delta(t) => {
                end.chars += t.len();
                if !on_delta(&t) {
                    return Err(NetError::Stopped);
                }
            }
            SseItem::Finish(f) => end.finish_reason = Some(f),
            SseItem::Tools(p) => add_pieces(&mut calls, p)?,
            SseItem::Done => return Ok((end, calls)),
            SseItem::Error => return Err(NetError::Upstream),
            SseItem::Skip => {}
        }
    }
}

/// Parses `{data: [{id}]}` into sorted, unique, checked ids.
pub fn parse_models(body: &[u8]) -> Result<Vec<String>, NetError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| NetError::BadAnswer)?;
    let data = v.get("data").and_then(Value::as_array).ok_or(NetError::BadAnswer)?;
    let mut ids: Vec<String> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .filter(|id| check_model(id).is_ok())
        .map(str::to_string)
        .take(MAX_MODELS)
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn check_status(s: u16) -> Result<(), NetError> {
    match s {
        200..=299 => Ok(()),
        401 | 403 => Err(NetError::Auth),
        404 => Err(NetError::NotOffered),
        429 => Err(NetError::RateLimited),
        _ => Err(NetError::Status(s)),
    }
}

pub(crate) fn map_io(e: std::io::Error) -> NetError {
    if e.kind() == std::io::ErrorKind::TimedOut {
        NetError::Timeout
    } else {
        NetError::Connect
    }
}

pub(crate) fn map_ureq(e: ureq::Error) -> NetError {
    match e {
        ureq::Error::Timeout(_) => NetError::Timeout,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => NetError::Tls,
        ureq::Error::Io(ref io) if io.kind() == std::io::ErrorKind::TimedOut => NetError::Timeout,
        ureq::Error::Io(ref io) if io.kind() == std::io::ErrorKind::InvalidData => NetError::Tls,
        _ => NetError::Connect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateways::Kind;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    fn msgs() -> Vec<Msg> {
        vec![Msg { role: "user".into(), content: "hi".into() }]
    }

    #[test]
    fn sse_lines_parse_to_fixed_items() {
        let d = r#"data: {"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#;
        assert_eq!(parse_sse_line(d).unwrap(), SseItem::Delta("Hel".into()));
        let f = r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        assert_eq!(parse_sse_line(f).unwrap(), SseItem::Finish("stop".into()));
        assert_eq!(parse_sse_line("data: [DONE]\n").unwrap(), SseItem::Done);
        assert_eq!(parse_sse_line(": keep-alive").unwrap(), SseItem::Skip);
        assert_eq!(parse_sse_line("").unwrap(), SseItem::Skip);
        assert_eq!(parse_sse_line(r#"data: {"choices":[]}"#).unwrap(), SseItem::Skip);
        assert_eq!(parse_sse_line(r#"data: {"error":{"message":"x"}}"#).unwrap(), SseItem::Error);
        assert_eq!(parse_sse_line("data: {not json"), Err(NetError::BadAnswer));
    }

    #[test]
    fn streamed_tool_calls_are_assembled_by_index() {
        let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\":\\\"a.py\\\"}\"}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"c2\",\"function\":{\"name\":\"search\",\"arguments\":{\"query\":\"x\"}}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
        let (end, calls) = read_stream_calls(body.as_bytes(), &mut |_| true).unwrap();
        assert_eq!(end.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(calls.len(), 2);
        assert_eq!(
            (calls[0].id.as_str(), calls[0].name.as_str(), calls[0].arguments.as_str()),
            ("call_1", "read_file", r#"{"path":"a.py"}"#)
        );
        assert_eq!((calls[1].name.as_str(), calls[1].arguments.as_str()), ("search", r#"{"query":"x"}"#));
        // An index past the cap is refused.
        let many =
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":9,\"function\":{\"name\":\"x\"}}]}}]}\n\n";
        assert_eq!(read_stream_calls(many.as_bytes(), &mut |_| true).err(), Some(NetError::TooLarge));
    }

    #[test]
    fn stream_reader_collects_stops_and_caps() {
        let body = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\" <b>world</b>\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
                    data: [DONE]\n\n";
        let mut got = String::new();
        let end = read_stream(body.as_bytes(), &mut |t| {
            got.push_str(t);
            true
        })
        .unwrap();
        // The answer is kept as text exactly as sent; the webview shows it as text, never HTML.
        assert_eq!(got, "Hello <b>world</b>");
        assert_eq!(end.finish_reason.as_deref(), Some("stop"));
        assert_eq!(read_stream(body.as_bytes(), &mut |_| false), Err(NetError::Stopped));
        let long = format!("data: {}\n", "x".repeat(MAX_LINE_BYTES + 10));
        assert_eq!(read_stream(long.as_bytes(), &mut |_| true), Err(NetError::TooLarge));
    }

    #[test]
    fn requests_are_checked_before_sending() {
        assert!(check_messages(&msgs()).is_ok());
        assert!(check_messages(&[]).is_err());
        assert!(check_messages(&[Msg { role: "tool".into(), content: "x".into() }]).is_err());
        assert!(check_messages(&[Msg { role: "user".into(), content: "x".repeat(MAX_MESSAGE_BYTES + 1) }]).is_err());
        assert!(check_model("llama3.1:8b").is_ok());
        assert!(check_model("").is_err() && check_model("a b").is_err());
    }

    #[test]
    fn model_lists_are_cleaned() {
        let b = br#"{"object":"list","data":[{"id":"b"},{"id":"a"},{"id":"a"},{"id":"bad id"},{"x":1}]}"#;
        assert_eq!(parse_models(b).unwrap(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(parse_models(b"[]"), Err(NetError::BadAnswer));
    }

    /// A one-request fake gateway on 127.0.0.1 (plain http, the dev case). Returns the base URL and a handle that
    /// yields the raw request it received.
    fn fake(status: &'static str, body: String) -> (String, thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let h = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            // Read headers, then the body by Content-Length.
            loop {
                let n = s.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let len = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string())
                        })
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    if req.len() >= i + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(resp.as_bytes()).unwrap();
            String::from_utf8_lossy(&req).to_string()
        });
        (url, h)
    }

    #[test]
    fn agent_step_sends_tools_and_returns_calls() {
        let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"list_dir\",\"arguments\":\"{\\\"path\\\":\\\"\\\"}\"}}]}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n".to_string();
        let (url, h) = fake("200 OK", body);
        let c = Client::new(&gw(&url), Some("p2p_secret_key_1".into())).unwrap();
        let tools = crate::agent::tool_defs();
        let msgs = vec![json!({"role": "user", "content": "list"})];
        let (end, calls) = c.agent_step("tiny", &msgs, &tools, &mut |_| true).unwrap();
        assert_eq!(end.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(
            calls,
            vec![ToolCall { id: "call_a".into(), name: "list_dir".into(), arguments: r#"{"path":""}"#.into() }]
        );
        let req = h.join().unwrap();
        assert!(req.contains("\"tools\":[") && req.contains("\"edit_file\"") && req.contains("\"stream\":true"));
    }

    fn gw(url: &str) -> Gateway {
        Gateway { id: "0123456789abcdef".into(), name: "dev".into(), url: url.into(), kind: Kind::Manual, ca_pem: None }
    }

    #[test]
    fn streams_a_chat_from_a_fake_gateway_with_the_key_in_the_header() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"4\"}}]}\n\ndata: [DONE]\n\n".to_string();
        let (url, h) = fake("200 OK", body);
        let c = Client::new(&gw(&url), Some("p2p_test_key_123".into())).unwrap();
        let mut got = String::new();
        let end = c
            .chat_stream("tiny", &msgs(), &mut |t| {
                got.push_str(t);
                true
            })
            .unwrap();
        assert_eq!(got, "4");
        assert_eq!(end.chars, 1);
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(
            req.contains("authorization: Bearer p2p_test_key_123")
                || req.contains("Authorization: Bearer p2p_test_key_123")
        );
        assert!(req.contains("\"stream\":true") && req.contains("\"model\":\"tiny\""));
        // SECURITY: the client's Debug output never shows the key.
        assert!(!format!("{c:?}").contains("p2p_test_key_123"));
    }

    #[test]
    fn status_codes_map_to_fixed_errors_and_redirects_are_not_followed() {
        for (status, want) in [
            ("401 Unauthorized", NetError::Auth),
            ("404 Not Found", NetError::NotOffered),
            ("429 Too Many Requests", NetError::RateLimited),
            ("500 Internal Server Error", NetError::Status(500)),
            ("302 Found", NetError::Status(302)),
        ] {
            let (url, h) = fake(status, String::new());
            let c = Client::new(&gw(&url), Some("p2p_test_key_123".into())).unwrap();
            assert_eq!(c.models().unwrap_err(), want, "{status}");
            h.join().unwrap();
        }
    }

    #[test]
    fn no_key_and_unreachable_gateways_fail_cleanly() {
        assert_eq!(Client::new(&gw("http://127.0.0.1:9"), None).unwrap_err(), NetError::NoKey);
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let c = Client::new(&gw(&format!("http://127.0.0.1:{port}")), Some("p2p_test_key_123".into())).unwrap();
        assert_eq!(c.models().unwrap_err(), NetError::Connect);
        assert!(!NetError::Connect.message().contains("127.0.0.1"));
    }

    #[test]
    fn a_pinned_ca_without_a_certificate_is_refused() {
        // The saved list only holds checked PEM (gateways::check_ca); this guards the client on its own too.
        let mut g = gw("https://gw.example.org");
        g.ca_pem = Some("not a pem file".into());
        assert_eq!(Client::new(&g, Some("p2p_test_key_123".into())).unwrap_err(), NetError::Tls);
    }
}
