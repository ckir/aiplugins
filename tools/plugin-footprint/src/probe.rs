//! Launch a plugin's MCP server and read what it advertises (spec §4.1, §4.4).
//!
//! Source parsing would miss drift from `rmcp` macro changes, `schemars` output
//! changes and doc-comment edits — all of which change what actually ships — so
//! the payload is taken from a live handshake instead. For the same reason the
//! responses are kept as raw `serde_json::Value`: deserialising into a typed
//! `Tool` would silently drop any field the type does not model, and measuring
//! what we can name rather than what the server sent is exactly the silent
//! disagreement with reality this approach exists to avoid.
//!
//! The conversation is deliberately blocking, with reader threads for the two
//! output pipes. There is no concurrency to manage here — it is one strictly
//! ordered request/response exchange — and threads plus `recv_timeout` give the
//! deadlines §4.4 requires without an async runtime.

use crate::canonical::canonical_len;
use crate::child_env::{child_env, inherited_env};
use crate::manifest::ServerSpec;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// JSON-RPC's "method not found". An MCP server need not implement every
/// optional list method, and saying so is an answer, not a failure.
const METHOD_NOT_FOUND: i64 = -32601;

/// How much of the server's stderr to keep for a failure message.
const STDERR_KEEP: usize = 4096;

/// Bounds on a single plugin's probe (spec §4.4).
#[derive(Debug, Clone)]
pub struct Limits {
    /// Longest wait for one response.
    pub per_method: Duration,
    /// Longest total wall-clock for one plugin, across every method.
    pub per_plugin: Duration,
    /// Most pages to follow for one list method.
    pub max_pages: usize,
    /// Most bytes to accumulate across one list method's pages.
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            per_method: Duration::from_secs(30),
            per_plugin: Duration::from_secs(120),
            max_pages: 64,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

/// How a probe ended.
///
/// A failure is never a zero. `Failed` and `TimedOut` mean the tier figures are
/// unknown, and §5 omits them rather than serialising zeros that a budget
/// ceiling would happily pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// The plugin declares no MCP server, so there was nothing to probe.
    ///
    /// A COMPLETE measurement, not a failed one, and distinct from `Ok` with an
    /// empty tool list on purpose. A skills-only or hooks-only plugin is
    /// ordinary (see `manifest::looks_like_a_plugin`) and its whole cost sits in
    /// the file-backed tiers; a server that WAS launched and answered
    /// `tools/list` with nothing is broken, and must not be able to satisfy a
    /// ceiling by costing nothing. Collapsing the two into one status is what
    /// made the gate reject the first while trying to catch the second.
    NoServer,
    Failed(String),
    TimedOut(String),
}

/// The result of probing one server.
#[derive(Debug)]
pub struct Outcome {
    pub status: Status,
    /// Raw `tools/list` entries, exactly as the server serialised them.
    pub tools: Vec<Value>,
    /// Raw `prompts/list` entries. Empty when the server does not implement it.
    pub prompts: Vec<Value>,
    pub binary: PathBuf,
    /// Whether the probe observed the server exit.
    ///
    /// The prober kills the process and then waits for it, so `true` means the
    /// OS confirmed the child is gone — not merely that the pipes were dropped
    /// and the process left to its own devices.
    ///
    /// Public, like every other field, so that tests can build an `Outcome`
    /// without this type growing a constructor that exists only for them.
    pub reaped: bool,
}

/// Launch `spec`'s server, complete the MCP handshake, and read what it lists.
///
/// Never returns an error: the outcome *is* the status. A caller that got a
/// document back still has to look at `status` before believing any number in
/// it, and making that unavoidable is the point.
pub fn probe(spec: &ServerSpec, limits: &Limits) -> Outcome {
    let env = child_env(&spec.env, &inherited_env());

    // An empty directory, not the caller's. Agent plugins read per-project
    // settings from files under the working directory — this repo's own
    // `.claude/re-ghidra-mcp-cc.local.md` is one — so a probe run from the repo
    // root measures a configured server while the same probe run elsewhere
    // measures an unconfigured one. MEASURED: that difference silently
    // invalidated a before/after comparison during this tool's development,
    // because both halves "passed" from the repo root. A footprint has to be a
    // property of the plugin, not of the directory someone happened to be in.
    let workdir = Workdir::new();

    let spawned = Command::new(&spec.command)
        .args(&spec.args)
        .current_dir(workdir.path())
        .env_clear()
        .envs(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            return Outcome {
                status: Status::Failed(format!("could not launch {}: {e}", spec.command.display())),
                tools: Vec::new(),
                prompts: Vec::new(),
                binary: spec.command.clone(),
                // Nothing was started, so nothing is outstanding.
                reaped: true,
            };
        }
    };

    let stderr = drain_stderr(&mut child);
    let outcome = converse(&mut child, limits);

    // Reap on EVERY outcome, the successful one included. Nothing in MCP obliges
    // a server to exit on stdin EOF, so a prober that merely drops its pipes
    // leaks one process per plugin per run — only when everything works, which
    // is the leak nobody attributes to the gate.
    let _ = child.kill();
    let reaped = child.wait().is_ok();

    let (status, tools, prompts) = match outcome {
        Ok((tools, prompts)) => (Status::Ok, tools, prompts),
        Err(status) => (with_stderr(status, &stderr), Vec::new(), Vec::new()),
    };

    Outcome {
        status,
        tools,
        prompts,
        binary: spec.command.clone(),
        reaped,
    }
}

/// A private empty directory to run a probed server in, removed when dropped.
struct Workdir {
    dir: PathBuf,
}

impl Workdir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-probe-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        // A failure here is not fatal: the directory may already exist from an
        // earlier probe in the same process, which is fine, and if it cannot be
        // created at all the spawn below reports the real problem.
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        // Best effort, and only after the child has been reaped — on Windows a
        // live process holds a lock on its working directory.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Attach whatever the server said on stderr to a failure, so the reader is not
/// left guessing why a binary they cannot see refused to talk.
fn with_stderr(status: Status, stderr: &Arc<Mutex<String>>) -> Status {
    let tail = stderr
        .lock()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if tail.is_empty() {
        return status;
    }
    match status {
        Status::Failed(why) => Status::Failed(format!("{why}; stderr: {tail}")),
        Status::TimedOut(why) => Status::TimedOut(format!("{why}; stderr: {tail}")),
        Status::Ok => Status::Ok,
        // Never produced here — nothing was launched, so there is no stderr to
        // attach. Spelled out rather than wildcarded so that a future status
        // added to this enum has to be considered at this site too.
        Status::NoServer => Status::NoServer,
    }
}

/// Read the child's stderr on its own thread.
///
/// Draining is not optional: an unread pipe fills, and a server that logs
/// enough would block writing to it and hang a probe that was otherwise fine.
fn drain_stderr(child: &mut Child) -> Arc<Mutex<String>> {
    let buffer = Arc::new(Mutex::new(String::new()));
    let Some(stderr) = child.stderr.take() else {
        return buffer;
    };
    let sink = Arc::clone(&buffer);
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let Ok(mut held) = sink.lock() else { return };
            if held.len() < STDERR_KEEP {
                held.push_str(&line);
                held.push('\n');
            }
        }
    });
    buffer
}

/// The whole conversation, or the status that ended it.
fn converse(child: &mut Child, limits: &Limits) -> Result<(Vec<Value>, Vec<Value>), Status> {
    let deadline = Instant::now() + limits.per_plugin;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Status::Failed("no stdin pipe on the probed server".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Status::Failed("no stdout pipe on the probed server".to_string()))?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                // A closed receiver means the probe has already finished.
                if tx.send(value).is_err() {
                    return;
                }
            }
        }
    });

    let mut conn = Conn {
        stdin: &mut stdin,
        rx,
        next_id: 1,
        limits,
        deadline,
    };

    conn.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "plugin-footprint", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;
    conn.notify("notifications/initialized")?;

    let tools = conn.list_all("tools/list", "tools")?;
    let prompts = conn.list_all("prompts/list", "prompts")?;

    Ok((tools, prompts))
}

struct Conn<'a> {
    stdin: &'a mut std::process::ChildStdin,
    rx: Receiver<Value>,
    next_id: i64,
    limits: &'a Limits,
    deadline: Instant,
}

impl Conn<'_> {
    fn notify(&mut self, method: &str) -> Result<(), Status> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    fn send(&mut self, message: &Value) -> Result<(), Status> {
        writeln!(self.stdin, "{message}")
            .and_then(|()| self.stdin.flush())
            .map_err(|e| Status::Failed(format!("writing to the server: {e}")))
    }

    /// Send a request and wait for the response carrying its id.
    ///
    /// Anything else on the channel — notifications, log messages — is skipped
    /// rather than treated as the answer.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Status> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;

        loop {
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_default();
            if remaining.is_zero() {
                return Err(Status::TimedOut(format!(
                    "the plugin budget expired waiting for {method}"
                )));
            }
            match self.rx.recv_timeout(self.limits.per_method.min(remaining)) {
                Ok(message) => {
                    if message.get("id") == Some(&json!(id)) {
                        return Ok(message);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Status::TimedOut(format!("no response to {method}")));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Status::Failed(format!(
                        "the server closed its output before answering {method}"
                    )));
                }
            }
        }
    }

    /// Read every page of a list method, following `nextCursor`.
    ///
    /// Bounded on both axes. Cursor-following is a loop driven by the thing being
    /// measured: a server returning a circular cursor, or arbitrarily many pages,
    /// would otherwise run until memory gave out — and a truncated page set would
    /// understate the footprint, which is the same wrong-direction error that
    /// following cursors at all exists to prevent.
    fn list_all(&mut self, method: &str, key: &str) -> Result<Vec<Value>, Status> {
        let mut items = Vec::new();
        let mut bytes = 0usize;
        let mut cursor: Option<String> = None;

        for page in 0.. {
            if page >= self.limits.max_pages {
                return Err(Status::Failed(format!(
                    "{method} exceeded the page cap of {} pages",
                    self.limits.max_pages
                )));
            }

            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let response = self.request(method, params)?;

            if let Some(error) = response.get("error") {
                let code = error.get("code").and_then(Value::as_i64);
                if code == Some(METHOD_NOT_FOUND) {
                    // Optional and not implemented: zero of this kind, measured.
                    return Ok(Vec::new());
                }
                return Err(Status::Failed(format!(
                    "{method} returned an error: {error}"
                )));
            }

            let Some(result) = response.get("result") else {
                return Err(Status::Failed(format!(
                    "{method} answered without a result"
                )));
            };

            bytes += canonical_len(result);
            if bytes > self.limits.max_bytes {
                return Err(Status::Failed(format!(
                    "{method} exceeded the size cap of {} bytes",
                    self.limits.max_bytes
                )));
            }

            // A missing or non-array key is a FAILURE, not an empty page. This is
            // the same rule the probe status follows, and the place it matters
            // most: a server answering under some other key would otherwise read
            // as "this plugin has no tools", sail through the gate as a
            // spectacular footprint reduction, and publish zero for a plugin that
            // costs whatever it costs. A wrong number people trust is worse than
            // a crash.
            let Some(page_items) = result.get(key).and_then(Value::as_array) else {
                return Err(Status::Failed(format!(
                    "{method} answered without a `{key}` array; a result we cannot read \
                     is not a measurement of zero"
                )));
            };
            items.extend(page_items.iter().cloned());

            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }

        Ok(items)
    }
}
