//! The relay: a plain HTTP/1.1 server on localhost that forwards each request to the real API
//! and streams the answer back byte for byte, reading the server-sent events as they pass.
//! Headers (the API key or OAuth token among them) go upstream untouched and are never kept.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json::Value;
use crate::card;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind { Text, Thinking, Tool(String), Skill(String) }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// The part of `cache_write` kept for an hour, which bills at 2× input rather than 1.25×.
    pub cache_write_1h: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    /// A request went upstream: its model, the context window it asked for in tokens, and the
    /// Claude Code session it came from.
    Start { id: u64, model: String, window: u64, session: String },
    Block { id: u64, kind: Kind },
    Delta { id: u64, kind: Kind, text: String },
    /// The request's usage so far (absolute, not an increment).
    Usage { id: u64, usage: Usage },
    Stop { id: u64, reason: String },
    End { id: u64, status: u16 },
    /// How hard the request asks the model to think: an effort level, or a thinking budget.
    Effort { id: u64, effort: String },
    /// A tool call's input, once whole, as one short line: a file's name, a command, a pattern.
    Arg { id: u64, arg: String },
    /// An edit Claude asked for, whole: its file's name and the lines it adds and removes.
    Diff { id: u64, file: String, added: u64, removed: u64 },
    /// Claude's todo list changed: the step in progress (counting from 1), how many there are,
    /// and what it says it's doing; None once no step is in progress.
    Todo { id: u64, step: Option<(usize, usize, String)> },
    /// The API turned the request away for a rate limit or overload; it says to retry in `secs`.
    Limit { id: u64, secs: f64 },
    /// A session's permission prompt is up, posted by Claude Code's `Notification` hook.
    Needs { session: String },
    Error(String),
}

pub type Sink = Arc<dyn Fn(Ev) + Send + Sync>;

/// Hop-by-hop headers, and the length and encoding the relay sets itself.
const SKIP: [&str; 6] = ["host", "connection", "keep-alive", "content-length", "transfer-encoding", "accept-encoding"];
const MAX_BODY: usize = 64 << 20;

pub fn serve(port: u16, upstream: String, sink: Sink) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    // Redirects go back to the client as they came: following one would resend `x-api-key` to its host.
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    std::thread::spawn(move || {
        for (id, conn) in listener.incoming().flatten().enumerate() {
            let (agent, upstream, sink) = (agent.clone(), upstream.clone(), sink.clone());
            let id = id as u64;
            std::thread::spawn(move || {
                let ended = Arc::new(AtomicBool::new(false));
                let status = relay(&conn, id, &agent, &upstream, &sink, &ended).unwrap_or_else(|e| { sink(Ev::Error(e)); 0 });
                if !ended.swap(true, Ordering::SeqCst) { sink(Ev::End { id, status }) }
                // The client reads to the close; the watcher's copy of the socket would hold it open.
                let _ = conn.shutdown(Shutdown::Both);
            });
        }
    });
    Ok(())
}

fn reply(mut conn: &TcpStream, status: &str, msg: &str) -> Result<u16, String> {
    let body = serde_json::json!({ "type": "error", "error": { "type": "proxy_error", "message": msg } }).to_string();
    write!(conn, "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
        .map_err(|e| e.to_string())?;
    Err(msg.to_string())
}

fn relay(mut conn: &TcpStream, id: u64, agent: &ureq::Agent, upstream: &str, sink: &Sink, ended: &Arc<AtomicBool>) -> Result<u16, String> {
    let mut reader = BufReader::new(conn);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next().map(String::from), parts.next().map(String::from)) else {
        return reply(conn, "400 Bad Request", "malformed request line");
    };
    let (mut headers, mut len) = (Vec::new(), 0usize);
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 { break }
        let Some((k, v)) = line.trim_end().split_once(':') else { break };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
        match k.as_str() {
            "content-length" => len = v.parse().unwrap_or(0),
            // shortcut: chunked request bodies are refused; Claude Code always sends a length.
            "transfer-encoding" => return reply(conn, "411 Length Required", "send a content-length"),
            k if SKIP.contains(&k) => {}
            _ => headers.push((k, v)),
        }
    }
    if len > MAX_BODY { return reply(conn, "413 Payload Too Large", "request body over 64 MB") }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;

    let req: Value = serde_json::from_slice(&body).unwrap_or_default();
    // Claude Code's hooks post what the relay can't see in the traffic.
    if path == "/atoll" {
        if req["notification_type"] == "permission_prompt" { sink(Ev::Needs { session: req["session_id"].as_str().unwrap_or("").to_string() }) }
        write!(conn, "HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").map_err(|e| e.to_string())?;
        return Ok(204)
    }
    let model = req["model"].as_str().unwrap_or(&path).to_string();
    let window = window(&model, &headers);
    sink(Ev::Start { id, model, window, session: session(&req, &headers) });
    // shortcut: read from the API's documented fields; Claude Code's own requests are unchecked.
    let thinking = &req["thinking"];
    let effort = req["output_config"]["effort"].as_str().map(String::from).or_else(|| match thinking["type"].as_str()? {
        "adaptive" => Some("adaptive".into()),
        "enabled" => thinking["budget_tokens"].as_u64().map(|b| format!("{}k thinking", b.div_ceil(1000))),
        _ => None,
    });
    if let Some(effort) = effort { sink(Ev::Effort { id, effort }) }
    watch_client(conn, id, sink, ended);

    let mut out = agent.request(&method, &format!("{upstream}{path}"));
    for (k, v) in &headers { out = out.set(k, v) }
    let resp = match if body.is_empty() { out.call() } else { out.send_bytes(&body) } {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return reply(conn, "502 Bad Gateway", &format!("upstream: {e}")),
    };

    let status = resp.status();
    if matches!(status, 429 | 529) && let Some(secs) = retry(&resp) { sink(Ev::Limit { id, secs }) }
    let mut head = format!("HTTP/1.1 {status} {}\r\n", resp.status_text());
    // A name comes once for each time it appears, and `all` gives every value under it.
    let mut names = resp.headers_names();
    names.sort();
    names.dedup();
    for name in names {
        if SKIP.contains(&name.to_ascii_lowercase().as_str()) { continue }
        for v in resp.all(&name) { head += &format!("{name}: {v}\r\n") }
    }
    // The body ends when the connection closes, so a stream passes through as it arrives.
    head += "connection: close\r\n\r\n";
    conn.write_all(head.as_bytes()).map_err(|e| e.to_string())?;

    let sse = resp.content_type() == "text/event-stream";
    let mut parser = Parser { id, ..Parser::default() };
    let (mut whole, mut buf, mut body) = (Vec::new(), vec![0u8; 16 << 10], resp.into_reader());
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break }
        // The client hung up: Claude Code was stopped, which is no error.
        if conn.write_all(&buf[..n]).and_then(|_| conn.flush()).is_err() { return Ok(status) }
        if sse { parser.feed(&buf[..n], &**sink) } else if whole.len() < MAX_BODY { whole.extend_from_slice(&buf[..n]) }
    }
    if !sse && let Ok(v) = serde_json::from_slice::<Value>(&whole) { parser.whole(&v, &**sink) }
    Ok(status)
}

/// Ends the request as soon as the client hangs up. Stopping Claude closes the connection, but
/// the relay only learns of it on its next write, and a long think can stream nothing for a while.
fn watch_client(conn: &TcpStream, id: u64, sink: &Sink, ended: &Arc<AtomicBool>) {
    let Ok(mut probe) = conn.try_clone() else { return };
    let (sink, ended) = (sink.clone(), ended.clone());
    std::thread::spawn(move || {
        // The whole request is read; what follows is only the close.
        while matches!(probe.read(&mut [0u8; 64]), Ok(n) if n > 0) {}
        if !ended.swap(true, Ordering::SeqCst) { sink(Ev::End { id, status: 0 }) }
    });
}

/// The context window a request asks for, in tokens: Claude Code asks for 1M with a beta header,
/// or a `[1m]` suffix on the model.
pub fn window(model: &str, headers: &[(String, String)]) -> u64 {
    let long = model.contains("[1m]") || headers.iter().any(|(k, v)| k == "anthropic-beta" && v.contains("context-1m"));
    if long { 1_000_000 } else { 200_000 }
}

/// Seconds until a refused request may go again: the reset of a subscription's usage window,
/// which the API sends as Unix seconds, or else `retry-after`.
fn retry(resp: &ureq::Response) -> Option<f64> {
    let num = |k: &str| resp.header(k).and_then(|v| v.trim().parse::<f64>().ok());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs_f64();
    num("anthropic-ratelimit-unified-reset").map(|reset| (reset - now).max(0.0)).or_else(|| num("retry-after"))
}

/// The Claude Code session a request belongs to, as its hooks report it: the
/// `x-claude-code-session-id` header, or else the session id older versions end
/// `metadata.user_id` with. Empty when the request names none, so such requests share one.
pub fn session(req: &Value, headers: &[(String, String)]) -> String {
    if let Some((_, v)) = headers.iter().find(|(k, _)| k == "x-claude-code-session-id") { return v.clone() }
    let user = req["metadata"]["user_id"].as_str().unwrap_or("");
    user.rsplit("session_").next().unwrap_or("").to_string()
}

/// Reads one response's server-sent events (or its whole JSON body) into [`Ev`]s.
#[derive(Default)]
pub struct Parser {
    pub id: u64,
    buf: Vec<u8>,
    blocks: Vec<Kind>,
    usage: Usage,
    /// A `Skill` tool call held back until its input names the skill: block index and input so far.
    skill: Option<(usize, String)>,
    /// A tool call gathered until whole, to read once: block index, tool and input so far.
    tool: Option<(usize, String, String)>,
}

/// What a whole tool call's input tells the island: its argument, an edit's lines, or the todo list's step.
fn gathered(id: u64, tool: &str, input: &Value, sink: &dyn Fn(Ev)) {
    let arg = card::summary(input);
    if !arg.is_empty() { sink(Ev::Arg { id, arg }) }
    if tool == "TodoWrite" { return sink(Ev::Todo { id, step: card::todo(input) }) }
    if let Some((file, added, removed)) = card::diff(tool, input) { sink(Ev::Diff { id, file, added, removed }) }
}

fn kind_of(block: &Value) -> Kind {
    match block["type"].as_str().unwrap_or("") {
        "thinking" | "redacted_thinking" => Kind::Thinking,
        "tool_use" | "server_tool_use" => match (block["name"].as_str().unwrap_or("tool"), block["input"]["skill"].as_str()) {
            ("Skill", Some(skill)) => Kind::Skill(skill.to_string()),
            (name, _) => Kind::Tool(name.to_string()),
        },
        _ => Kind::Text,
    }
}

impl Parser {
    pub fn feed(&mut self, bytes: &[u8], sink: &dyn Fn(Ev)) {
        self.buf.extend_from_slice(bytes);
        while let Some(nl) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=nl).collect();
            if let Some(data) = line.strip_prefix(b"data:")
                && let Ok(v) = serde_json::from_slice::<Value>(data)
            {
                self.event(&v, sink)
            }
        }
    }

    fn event(&mut self, v: &Value, sink: &dyn Fn(Ev)) {
        let id = self.id;
        match v["type"].as_str().unwrap_or("") {
            "message_start" => self.usage(&v["message"]["usage"], sink),
            "content_block_start" => {
                let (i, kind) = (v["index"].as_u64().unwrap_or(0) as usize, kind_of(&v["content_block"]));
                if self.blocks.len() <= i { self.blocks.resize(i + 1, Kind::Text) }
                self.blocks[i] = kind.clone();
                if kind == Kind::Tool("Skill".into()) { self.skill = Some((i, String::new())); return }
                if let Kind::Tool(name) = &kind { self.tool = Some((i, name.clone(), String::new())) }
                sink(Ev::Block { id, kind });
            }
            "content_block_delta" => {
                let d = &v["delta"];
                let text = d["text"].as_str().or(d["thinking"].as_str()).or(d["partial_json"].as_str()).unwrap_or("");
                if let Some((i, input)) = &mut self.skill && v["index"].as_u64() == Some(*i as u64) { return input.push_str(text) }
                if let Some((i, _, input)) = &mut self.tool && v["index"].as_u64() == Some(*i as u64) { input.push_str(text) }
                let kind = self.blocks.get(v["index"].as_u64().unwrap_or(0) as usize).cloned().unwrap_or(Kind::Text);
                if !text.is_empty() { sink(Ev::Delta { id, kind, text: text.to_string() }) }
            }
            "content_block_stop" => {
                if let Some((_, tool, input)) = self.tool.take_if(|(i, ..)| v["index"].as_u64() == Some(*i as u64)) {
                    let input: Value = serde_json::from_str(&input).unwrap_or_default();
                    gathered(id, &tool, &input, sink);
                }
                let Some((i, input)) = self.skill.take_if(|(i, _)| v["index"].as_u64() == Some(*i as u64)) else { return };
                let input: Value = serde_json::from_str(&input).unwrap_or_default();
                let kind = Kind::Skill(input["skill"].as_str().unwrap_or("skill").to_string());
                self.blocks[i] = kind.clone();
                sink(Ev::Block { id, kind: kind.clone() });
                sink(Ev::Delta { id, kind, text: input.to_string() });
                gathered(id, "Skill", &input, sink);
            }
            "message_delta" => {
                if let Some(r) = v["delta"]["stop_reason"].as_str() { sink(Ev::Stop { id, reason: r.to_string() }) }
                self.usage(&v["usage"], sink);
            }
            "error" => sink(Ev::Error(v["error"]["message"].as_str().unwrap_or("stream error").to_string())),
            _ => {}
        }
    }

    /// Usage fields arrive piecemeal (input at the start, output at the end); keep what each sends.
    fn usage(&mut self, u: &Value, sink: &dyn Fn(Ev)) {
        if !u.is_object() { return }
        let n = |k: &str| u[k].as_u64();
        let us = &mut self.usage;
        for (field, key) in [(&mut us.input, "input_tokens"), (&mut us.output, "output_tokens"),
            (&mut us.cache_read, "cache_read_input_tokens"), (&mut us.cache_write, "cache_creation_input_tokens")] {
            if let Some(v) = n(key) { *field = v }
        }
        if let Some(v) = u["cache_creation"]["ephemeral_1h_input_tokens"].as_u64() { us.cache_write_1h = v }
        sink(Ev::Usage { id: self.id, usage: self.usage });
    }

    /// A non-streaming answer: replay its blocks as if they had streamed.
    fn whole(&mut self, v: &Value, sink: &dyn Fn(Ev)) {
        let id = self.id;
        if v["type"] == "error" { return sink(Ev::Error(v["error"]["message"].as_str().unwrap_or("error").to_string())) }
        for b in v["content"].as_array().into_iter().flatten() {
            let kind = kind_of(b);
            sink(Ev::Block { id, kind: kind.clone() });
            let text = b["text"].as_str().or(b["thinking"].as_str()).map(String::from).unwrap_or_else(|| b["input"].to_string());
            sink(Ev::Delta { id, kind: kind.clone(), text });
            if let Kind::Tool(tool) = kind { gathered(id, &tool, &b["input"], sink) }
        }
        if let Some(r) = v["stop_reason"].as_str() { sink(Ev::Stop { id, reason: r.to_string() }) }
        self.usage(&v["usage"], sink);
    }
}
