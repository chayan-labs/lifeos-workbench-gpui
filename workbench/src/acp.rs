//! ACP client (issue #15): speaks the Agent Client Protocol - newline-
//! delimited JSON-RPC 2.0 over the agent's stdio - so Claude Code, the
//! existing harness, or any ACP agent plugs in interchangeably. We do NOT
//! write a model; we spawn the agent the user configured and converse.
//!
//! The one deliberate deviation from a stock client: `fs/write_text_file`
//! requests are NOT applied. They are staged as reviewable diffs (issue
//! #16) and the JSON-RPC response is held open until the user accepts
//! (write + success) or rejects (error) - so the agent's own view of the
//! session stays truthful. The official `agent-client-protocol` SDK wraps
//! the wire in an async connection framework that cannot hold a response
//! across UI interaction in our sync event loop, hence the thin peer here.

use crate::diff::{self, Hunk};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// One conversation entry, in arrival order.
#[derive(Clone, Debug)]
pub enum Entry {
    User(String),
    Agent(String),
    ToolCall(String),
    Info(String),
}

/// An agent-proposed file edit awaiting review. `respond_id` is the held
/// JSON-RPC request id answered on accept/reject.
#[derive(Clone, Debug)]
pub struct ProposedEdit {
    pub path: PathBuf,
    pub old_text: String,
    pub new_text: String,
    pub hunks: Vec<Hunk>,
    pub accepted: Vec<usize>,
    respond_id: i64,
}

#[derive(Default)]
pub struct Conversation {
    pub entries: Vec<Entry>,
    pub edits: Vec<ProposedEdit>,
    pub busy: bool,
}

type Shared = Arc<Mutex<Conversation>>;
type Pending = Arc<Mutex<HashMap<i64, mpsc::Sender<Value>>>>;

pub struct AcpAgent {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pending: Pending,
    pub conversation: Shared,
    next_id: AtomicI64,
    session_id: Mutex<Option<String>>,
    cwd: PathBuf,
    mcp_servers: Vec<Value>,
    child: Option<Child>,
}

fn write_line(w: &mut dyn Write, msg: &Value) -> std::io::Result<()> {
    writeln!(w, "{msg}")?;
    w.flush()
}

impl AcpAgent {
    pub fn from_streams(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        cwd: &Path,
        mcp_servers: Vec<Value>,
    ) -> Self {
        let pending: Pending = Arc::default();
        let conversation: Shared = Arc::default();
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));
        let (p, c, w) = (pending.clone(), conversation.clone(), writer.clone());
        std::thread::spawn(move || {
            let mut lines = BufReader::new(reader).lines();
            while let Some(Ok(line)) = lines.next() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(msg) => route(&p, &c, &w, msg),
                    Err(_) => continue,
                }
            }
        });
        Self {
            writer,
            pending,
            conversation,
            next_id: AtomicI64::new(1),
            session_id: Mutex::new(None),
            cwd: cwd.to_path_buf(),
            mcp_servers,
            child: None,
        }
    }

    /// Spawn the configured agent (`WORKBENCH_AGENT_CMD`, default
    /// `claude-code-acp`) and run initialize + session/new.
    pub fn spawn(command: &str, cwd: &Path, mcp_servers: Vec<Value>) -> Option<Self> {
        let mut parts = command.split_whitespace();
        let program = parts.next()?;
        let mut child = Command::new(program)
            .args(parts)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdout = child.stdout.take()?;
        let stdin = child.stdin.take()?;
        let mut agent = Self::from_streams(Box::new(stdout), Box::new(stdin), cwd, mcp_servers);
        agent.child = Some(child);
        agent.handshake()?;
        Some(agent)
    }

    fn handshake(&self) -> Option<()> {
        self.request(
            "initialize",
            json!({"protocolVersion": 1, "clientCapabilities":
                   {"fs": {"readTextFile": true, "writeTextFile": true}}}),
            Duration::from_secs(20),
        )?;
        let session = self.request(
            "session/new",
            json!({"cwd": self.cwd.display().to_string(), "mcpServers": self.mcp_servers}),
            Duration::from_secs(20),
        )?;
        let id = session["sessionId"].as_str()?.to_string();
        *self.session_id.lock().ok()? = Some(id);
        Some(())
    }

    fn request(&self, method: &str, params: Value, timeout: Duration) -> Option<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().ok()?.insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        {
            let mut w = self.writer.lock().ok()?;
            write_line(&mut **w, &msg).ok()?;
        }
        let reply = rx.recv_timeout(timeout).ok();
        self.pending.lock().ok()?.remove(&id);
        reply.and_then(|m| m.get("result").cloned())
    }

    /// Send a user prompt. Non-blocking: the turn runs on a thread; chunks
    /// stream into the conversation, `busy` clears when the turn ends.
    pub fn prompt(&self, text: &str) {
        let Some(session_id) = self.session_id.lock().ok().and_then(|s| s.clone()) else {
            if let Ok(mut c) = self.conversation.lock() {
                c.entries.push(Entry::Info("agent not connected".into()));
            }
            return;
        };
        if let Ok(mut c) = self.conversation.lock() {
            c.entries.push(Entry::User(text.to_string()));
            c.busy = true;
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id, tx);
        }
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": "session/prompt",
                         "params": {"sessionId": session_id,
                                    "prompt": [{"type": "text", "text": text}]}});
        if let Ok(mut w) = self.writer.lock() {
            let _ = write_line(&mut **w, &msg);
        }
        let conversation = self.conversation.clone();
        let pending = self.pending.clone();
        std::thread::spawn(move || {
            // Agent turns can be long; an hour is effectively "until done".
            let _ = rx.recv_timeout(Duration::from_secs(3600));
            if let Ok(mut p) = pending.lock() {
                p.remove(&id);
            }
            if let Ok(mut c) = conversation.lock() {
                c.busy = false;
            }
        });
    }

    /// Toggle one hunk of a staged edit.
    pub fn toggle_hunk(&self, edit_idx: usize, hunk_idx: usize) {
        if let Ok(mut c) = self.conversation.lock() {
            if let Some(edit) = c.edits.get_mut(edit_idx) {
                match edit.accepted.iter().position(|&h| h == hunk_idx) {
                    Some(i) => {
                        edit.accepted.remove(i);
                    }
                    None if hunk_idx < edit.hunks.len() => edit.accepted.push(hunk_idx),
                    None => {}
                }
            }
        }
    }

    /// Resolve a staged edit: write the accepted hunks (if any) and answer
    /// the held `fs/write_text_file` request. Returns the written path.
    pub fn resolve_edit(&self, edit_idx: usize, accept: bool) -> Option<PathBuf> {
        let edit = {
            let mut c = self.conversation.lock().ok()?;
            if edit_idx >= c.edits.len() {
                return None;
            }
            c.edits.remove(edit_idx)
        };
        let written = if accept {
            let text = diff::apply(&edit.old_text, &edit.hunks, &edit.accepted);
            std::fs::write(&edit.path, text).ok()?;
            Some(edit.path.clone())
        } else {
            None
        };
        let response = if accept {
            json!({"jsonrpc": "2.0", "id": edit.respond_id, "result": null})
        } else {
            json!({"jsonrpc": "2.0", "id": edit.respond_id,
                   "error": {"code": -32000, "message": "user rejected the edit"}})
        };
        if let Ok(mut w) = self.writer.lock() {
            let _ = write_line(&mut **w, &response);
        }
        if let Ok(mut c) = self.conversation.lock() {
            let verdict = if accept { "accepted" } else { "rejected" };
            c.entries.push(Entry::Info(format!(
                "{verdict} edit: {}",
                edit.path.display()
            )));
        }
        written
    }
}

/// Handle one incoming message from the agent.
fn route(
    pending: &Pending,
    conversation: &Shared,
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    msg: Value,
) {
    let method = msg.get("method").and_then(Value::as_str);
    let id = msg.get("id").and_then(Value::as_i64);
    match (method, id) {
        // Response to one of our requests.
        (None, Some(id)) => {
            if let Some(tx) = pending.lock().ok().and_then(|mut p| p.remove(&id)) {
                let _ = tx.send(msg);
            }
        }
        (Some("session/update"), _) => on_session_update(conversation, &msg["params"]),
        (Some("fs/write_text_file"), Some(id)) => {
            stage_edit(conversation, id, &msg["params"]);
        }
        (Some("fs/read_text_file"), Some(id)) => {
            let content = msg["params"]["path"]
                .as_str()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .unwrap_or_default();
            respond(
                writer,
                json!({"jsonrpc": "2.0", "id": id, "result": {"content": content}}),
            );
        }
        // Outward actions stay human-gated elsewhere; for tool permission
        // we surface the auto-selection in the transcript.
        (Some("session/request_permission"), Some(id)) => {
            let option = msg["params"]["options"][0]["optionId"]
                .as_str()
                .unwrap_or("allow")
                .to_string();
            if let Ok(mut c) = conversation.lock() {
                c.entries
                    .push(Entry::Info(format!("auto-approved permission ({option})")));
            }
            respond(
                writer,
                json!({"jsonrpc": "2.0", "id": id,
                       "result": {"outcome": {"outcome": "selected", "optionId": option}}}),
            );
        }
        _ => {}
    }
}

fn respond(writer: &Arc<Mutex<Box<dyn Write + Send>>>, msg: Value) {
    if let Ok(mut w) = writer.lock() {
        let _ = write_line(&mut **w, &msg);
    }
}

fn on_session_update(conversation: &Shared, params: &Value) {
    let update = &params["update"];
    let Ok(mut c) = conversation.lock() else {
        return;
    };
    match update["sessionUpdate"].as_str() {
        Some("agent_message_chunk") => {
            let text = update["content"]["text"].as_str().unwrap_or_default();
            // Coalesce consecutive chunks into one agent entry.
            if let Some(Entry::Agent(prev)) = c.entries.last_mut() {
                prev.push_str(text);
            } else {
                c.entries.push(Entry::Agent(text.to_string()));
            }
        }
        Some("tool_call") => {
            let title = update["title"].as_str().unwrap_or("tool call");
            c.entries.push(Entry::ToolCall(title.to_string()));
        }
        _ => {}
    }
}

/// Turn an agent write request into a reviewable diff (all hunks
/// pre-accepted; the user deselects before accepting, or rejects outright).
fn stage_edit(conversation: &Shared, respond_id: i64, params: &Value) {
    let (Some(path), Some(content)) = (params["path"].as_str(), params["content"].as_str()) else {
        return;
    };
    let path = PathBuf::from(path);
    let old_text = std::fs::read_to_string(&path).unwrap_or_default();
    let hunks = diff::diff(&old_text, content);
    let accepted = (0..hunks.len()).collect();
    if let Ok(mut c) = conversation.lock() {
        c.entries.push(Entry::Info(format!(
            "proposed edit: {} ({} hunks)",
            path.display(),
            hunks.len()
        )));
        c.edits.push(ProposedEdit {
            path,
            old_text,
            new_text: content.to_string(),
            hunks,
            accepted,
            respond_id,
        });
    }
}

impl Drop for AcpAgent {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake ACP agent: handshake, streams two chunks, proposes a file edit
    /// via fs/write_text_file, ends the turn after the edit is resolved.
    const FAKE_AGENT: &str = r#"
import json, sys

def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()

pending_prompt = None
for line in sys.stdin:
    msg = json.loads(line)
    m = msg.get("method")
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"],
              "result": {"protocolVersion": 1, "agentCapabilities": {}}})
    elif m == "session/new":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": "s1"}})
    elif m == "session/prompt":
        pending_prompt = msg["id"]
        sid = msg["params"]["sessionId"]
        for chunk in ["I will ", "edit the file."]:
            send({"jsonrpc": "2.0", "method": "session/update",
                  "params": {"sessionId": sid,
                             "update": {"sessionUpdate": "agent_message_chunk",
                                        "content": {"type": "text", "text": chunk}}}})
        target = msg["params"]["prompt"][0]["text"]
        send({"jsonrpc": "2.0", "id": 100, "method": "fs/write_text_file",
              "params": {"sessionId": sid, "path": target,
                         "content": "a\nB\nc\nd\n"}})
    elif msg.get("id") == 100:
        send({"jsonrpc": "2.0", "id": pending_prompt,
              "result": {"stopReason": "end_turn"}})
"#;

    fn spawn_fake(cwd: &Path) -> Option<AcpAgent> {
        let script = std::env::temp_dir().join(format!("wb_fake_acp_{}.py", std::process::id()));
        std::fs::write(&script, FAKE_AGENT).unwrap();
        AcpAgent::spawn(&format!("python3 {}", script.display()), cwd, Vec::new())
    }

    fn wait_until(deadline_ms: u64, mut done: impl FnMut() -> bool) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_millis(deadline_ms);
        while std::time::Instant::now() < deadline {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn converses_stages_agent_edit_and_accept_applies_hunks() {
        let dir = std::env::temp_dir();
        let target = dir.join(format!("wb_acp_target_{}.txt", std::process::id()));
        std::fs::write(&target, "a\nb\nc\n").unwrap();
        let Some(agent) = spawn_fake(&dir) else {
            eprintln!("python3 unavailable - skipping");
            return;
        };

        // The fake edits whatever file path we prompt with.
        agent.prompt(&target.display().to_string());
        assert!(
            wait_until(5000, || {
                let c = agent.conversation.lock().unwrap();
                !c.edits.is_empty()
            }),
            "edit must be staged, not written"
        );
        // Nothing on disk yet - the response is held for review.
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a\nb\nc\n");
        {
            let c = agent.conversation.lock().unwrap();
            let text: Vec<String> = c
                .entries
                .iter()
                .filter_map(|e| match e {
                    Entry::Agent(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(text.join(""), "I will edit the file.");
            assert_eq!(c.edits[0].hunks.len(), 2);
        }

        // Reject the trailing insert hunk, accept the rest.
        agent.toggle_hunk(0, 1);
        let written = agent.resolve_edit(0, true).expect("write");
        assert_eq!(written, target);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a\nB\nc\n");

        // The held response was answered, so the fake ends the turn.
        assert!(
            wait_until(5000, || !agent.conversation.lock().unwrap().busy),
            "turn must end after the edit resolves"
        );
        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn rejecting_an_edit_leaves_the_file_untouched() {
        let dir = std::env::temp_dir();
        let target = dir.join(format!("wb_acp_reject_{}.txt", std::process::id()));
        std::fs::write(&target, "a\nb\nc\n").unwrap();
        let Some(agent) = spawn_fake(&dir) else {
            eprintln!("python3 unavailable - skipping");
            return;
        };
        agent.prompt(&target.display().to_string());
        assert!(wait_until(5000, || !agent
            .conversation
            .lock()
            .unwrap()
            .edits
            .is_empty()));
        assert!(agent.resolve_edit(0, false).is_none());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a\nb\nc\n");
        std::fs::remove_file(&target).ok();
    }
}
