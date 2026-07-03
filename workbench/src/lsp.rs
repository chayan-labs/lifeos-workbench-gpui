//! Minimal LSP client over stdio (issue #8): diagnostics, hover, go-to-def.
//! helix-lsp's `Client` is registry-coupled to helix-view's `Editor`, so
//! embedding it would fork more than it reuses; this is the thin JSON-RPC
//! transport instead. Servers come from PATH (rust-analyzer for .rs). Absent
//! server = silent no-op, never an error surfaced to the editor.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// One diagnostic, reduced to what the gutter renders.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub line: usize,
    /// LSP DiagnosticSeverity: 1 = error, 2 = warning, 3 = info, 4 = hint.
    pub severity: u8,
    pub message: String,
}

type Diagnostics = Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>;
type Pending = Arc<Mutex<HashMap<i64, mpsc::Sender<Value>>>>;

pub struct LspClient {
    writer: Mutex<Box<dyn Write + Send>>,
    pending: Pending,
    diagnostics: Diagnostics,
    next_id: AtomicI64,
    child: Option<Child>,
}

/// `Content-Length: N\r\n\r\n{...}` framing.
pub fn write_message(w: &mut dyn Write, msg: &Value) -> std::io::Result<()> {
    let body = msg.to_string();
    write!(w, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    w.flush()
}

pub fn read_message(r: &mut dyn BufRead) -> std::io::Result<Value> {
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            length = v
                .trim()
                .parse()
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        }
    }
    let mut body = vec![0u8; length];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))
}

fn uri_of(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn path_of(uri: &str) -> PathBuf {
    PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri))
}

impl LspClient {
    /// Build from raw streams (tests use pipes; `spawn` uses child stdio).
    pub fn from_streams(reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) -> Self {
        let pending: Pending = Arc::default();
        let diagnostics: Diagnostics = Arc::default();
        let (p, d) = (pending.clone(), diagnostics.clone());
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader);
            while let Ok(msg) = read_message(&mut reader) {
                route(&p, &d, msg);
            }
        });
        Self {
            writer: Mutex::new(writer),
            pending,
            diagnostics,
            next_id: AtomicI64::new(1),
            child: None,
        }
    }

    /// Spawn a language server from PATH rooted at `root` and initialize it.
    pub fn spawn(program: &str, root: &Path) -> Option<Self> {
        let mut child = Command::new(program)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdout = child.stdout.take()?;
        let stdin = child.stdin.take()?;
        let mut client = Self::from_streams(Box::new(stdout), Box::new(stdin));
        client.child = Some(child);
        client.initialize(root)?;
        Some(client)
    }

    pub fn initialize(&self, root: &Path) -> Option<Value> {
        let result = self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": uri_of(root),
                "capabilities": {"textDocument": {
                    "hover": {"contentFormat": ["plaintext", "markdown"]},
                    "publishDiagnostics": {}
                }}
            }),
            Duration::from_secs(10),
        )?;
        self.notify("initialized", json!({}));
        Some(result)
    }

    pub fn notify(&self, method: &str, params: Value) {
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if let Ok(mut w) = self.writer.lock() {
            let _ = write_message(&mut **w, &msg);
        }
    }

    /// Blocking request with timeout; `None` on timeout or server error.
    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Option<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().ok()?.insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        {
            let mut w = self.writer.lock().ok()?;
            write_message(&mut **w, &msg).ok()?;
        }
        let reply = rx.recv_timeout(timeout).ok();
        self.pending.lock().ok()?.remove(&id);
        reply
            .and_then(|m| m.get("result").cloned())
            .filter(|r| !r.is_null())
    }

    pub fn did_open(&self, path: &Path, text: &str, language_id: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri_of(path), "languageId": language_id,
                   "version": 1, "text": text}}),
        );
    }

    pub fn did_change(&self, path: &Path, text: &str, version: i64) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri_of(path), "version": version},
                   "contentChanges": [{"text": text}]}),
        );
    }

    pub fn diagnostics_for(&self, path: &Path) -> Vec<Diagnostic> {
        self.diagnostics
            .lock()
            .map(|d| d.get(&uri_of(path)).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    /// Hover text at (line, col), first paragraph only.
    pub fn hover(&self, path: &Path, line: usize, col: usize) -> Option<String> {
        let result = self.request(
            "textDocument/hover",
            text_document_position(path, line, col),
            Duration::from_secs(5),
        )?;
        let contents = &result["contents"];
        let text = contents["value"]
            .as_str()
            .or_else(|| contents.as_str())
            .or_else(|| contents[0]["value"].as_str())
            .or_else(|| contents[0].as_str())?;
        text.lines()
            .find(|l| !l.trim().is_empty())
            .map(String::from)
    }

    /// First go-to-definition target as (path, line).
    pub fn definition(&self, path: &Path, line: usize, col: usize) -> Option<(PathBuf, usize)> {
        let result = self.request(
            "textDocument/definition",
            text_document_position(path, line, col),
            Duration::from_secs(5),
        )?;
        let loc = if result.is_array() {
            &result[0]
        } else {
            &result
        };
        let uri = loc["uri"].as_str().or_else(|| loc["targetUri"].as_str())?;
        let range = if loc["range"].is_object() {
            &loc["range"]
        } else {
            &loc["targetSelectionRange"]
        };
        let line = range["start"]["line"].as_u64()? as usize;
        Some((path_of(uri), line))
    }
}

fn text_document_position(path: &Path, line: usize, col: usize) -> Value {
    json!({"textDocument": {"uri": uri_of(path)},
           "position": {"line": line, "character": col}})
}

/// Route one incoming message: responses to their waiter, diagnostics into
/// the shared store, everything else dropped.
fn route(pending: &Pending, diagnostics: &Diagnostics, msg: Value) {
    if let Some(id) = msg.get("id").and_then(Value::as_i64) {
        if msg.get("result").is_some() || msg.get("error").is_some() {
            if let Some(tx) = pending.lock().ok().and_then(|mut p| p.remove(&id)) {
                let _ = tx.send(msg);
            }
            return;
        }
    }
    if msg.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
        let params = &msg["params"];
        let Some(uri) = params["uri"].as_str() else {
            return;
        };
        let diags = params["diagnostics"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|d| {
                        Some(Diagnostic {
                            line: d["range"]["start"]["line"].as_u64()? as usize,
                            severity: d["severity"].as_u64().unwrap_or(1) as u8,
                            message: d["message"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Ok(mut store) = diagnostics.lock() {
            store.insert(uri.to_string(), diags);
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Which server binary serves a file, by extension.
pub fn server_for(path: &Path) -> Option<(&'static str, &'static str)> {
    match path.extension()?.to_str()? {
        "rs" => Some(("rust-analyzer", "rust")),
        "py" => Some(("pylsp", "python")),
        "js" | "mjs" | "jsx" | "ts" | "tsx" => Some(("typescript-language-server", "javascript")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn codec_round_trips_content_length_framing() {
        let msg = json!({"jsonrpc": "2.0", "id": 7, "method": "x", "params": {"a": 1}});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();
        assert!(String::from_utf8_lossy(&buf).starts_with("Content-Length:"));
        let mut reader = BufReader::new(Cursor::new(buf));
        assert_eq!(read_message(&mut reader).unwrap(), msg);
    }

    /// End-to-end against a fake stdio LSP server (python3): initialize,
    /// hover, publishDiagnostics. Skips when python3 is unavailable.
    #[test]
    fn client_talks_to_a_fake_server_end_to_end() {
        let fake = r#"
import json, sys

def send(msg):
    body = json.dumps(msg)
    sys.stdout.write(f"Content-Length: {len(body)}\r\n\r\n{body}")
    sys.stdout.flush()

while True:
    line = sys.stdin.readline()
    if not line:
        break
    length = 0
    while line.strip():
        if line.lower().startswith("content-length:"):
            length = int(line.split(":")[1])
        line = sys.stdin.readline()
    msg = json.loads(sys.stdin.read(length))
    m = msg.get("method")
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"capabilities": {}}})
    elif m == "textDocument/didOpen":
        uri = msg["params"]["textDocument"]["uri"]
        send({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
              "params": {"uri": uri, "diagnostics": [
                  {"range": {"start": {"line": 2, "character": 0},
                             "end": {"line": 2, "character": 1}},
                   "message": "fake warning"}]}})
    elif m == "textDocument/hover":
        send({"jsonrpc": "2.0", "id": msg["id"],
              "result": {"contents": {"kind": "plaintext", "value": "fn fake()"}}})
"#;
        let script = std::env::temp_dir().join(format!("wb_fake_lsp_{}.py", std::process::id()));
        std::fs::write(&script, fake).unwrap();
        let child = Command::new("python3")
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            eprintln!("python3 unavailable - skipping");
            return;
        };
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut client = LspClient::from_streams(Box::new(stdout), Box::new(stdin));
        client.child = Some(child);

        let root = std::env::temp_dir();
        assert!(
            client.initialize(&root).is_some(),
            "initialize must respond"
        );

        let file = root.join("fake.rs");
        client.did_open(&file, "fn main() {}\n", "rust");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let diags = client.diagnostics_for(&file);
            if !diags.is_empty() {
                assert_eq!(diags[0].line, 2);
                assert_eq!(diags[0].message, "fake warning");
                break;
            }
            assert!(std::time::Instant::now() < deadline, "no diagnostics");
            std::thread::sleep(Duration::from_millis(20));
        }

        let hover = client.hover(&file, 0, 3).expect("hover");
        assert_eq!(hover, "fn fake()");
        std::fs::remove_file(script).ok();
    }
}
