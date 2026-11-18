// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The MCP server: four tools over the engine, the client's roots (7.1) and the start-up
//! archiving of 8.5.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::service::{NotificationContext, Peer};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};

use crate::archive;
use crate::clock::SystemClock;
use crate::error::Error;

type EResult<T> = crate::error::Result<T>;
use crate::host::Host;
use crate::ops::{self, CreateArgs, GetArgs, ListHeader, UpdateArgs};
use crate::resolve::{locate, start_dir, Entry, Env, Store};
use crate::roots::{Action, CallGate, RootState, Settlement, ROOTS_WINDOW};
use crate::store::Tuning;

const MAX_PROTOCOL: ProtocolVersion = ProtocolVersion::V_2025_11_25;

/// What a host's configuration cannot reach: the roots window, shortened by tests only.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub roots_window: Duration,
}

impl Default for ServeOptions {
    fn default() -> ServeOptions {
        ServeOptions {
            roots_window: ROOTS_WINDOW,
        }
    }
}

/// Serves on standard input and output until the client goes away.
///
/// The not-initialized guard relies on rmcp spawning the `initialized` handler before the task of
/// the next message, which holds on a `current_thread` runtime (all four binaries use one); on a
/// multi-thread runtime the guard could refuse a legitimate first call.
pub async fn serve(host: Host, version: &str, env: Env) -> EResult<()> {
    serve_with(host, version, env, ServeOptions::default()).await
}

/// `serve` with the options that tests change. The runtime note on [`serve`] applies.
pub async fn serve_with(host: Host, version: &str, env: Env, opts: ServeOptions) -> EResult<()> {
    let stopped = |e: &dyn std::fmt::Display| Error::Server {
        reason: e.to_string(),
    };
    let running = RepoTasksServer::new(host, version, env, opts)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| stopped(&e))?;
    running.waiting().await.map_err(|e| stopped(&e))?;
    Ok(())
}

/// The four tool names with their descriptions, in registration order.
pub fn tool_descriptions() -> Vec<(&'static str, &'static str)> {
    vec![
        ("task_create", "Create one to 50 tasks in this repository's shared task list; later tasks may name earlier ones of the same call in blocked_by_earlier. Replies one line per task. Use these tools for work that must outlive the session or be seen by another agent."),
        ("task_get", "Read one task by its integer id: status, owner, blockers, what it blocks, times and description; a task that has left the list is answered from the archive. Use these tools for work that must outlive the session or be seen by another agent."),
        ("task_update", "Change a task by its integer id: status, subject, description, owner, blockers, or delete: true for a task that will not be done. Any agent may update any task. Use these tools for work that must outlive the session or be seen by another agent."),
        ("task_list", "List every task in this repository's shared task list, one line each, under a first line naming the list. Use these tools for work that must outlive the session or be seen by another agent."),
    ]
}

/// Why a call cannot go on.
enum Fail {
    /// `notifications/initialized` has not arrived: a JSON-RPC error, no list touched.
    NotInitialized,
    /// A refused task or an unusable list: a tool result with `isError`.
    Tool(Error),
}

struct Shared {
    host: Host,
    version: String,
    env: Env,
    tuning: Tuning,
    window: Duration,
    /// `None` until `notifications/initialized` arrives; the state machine lives behind this lock
    /// and the lock is never held across an await.
    roots: Mutex<Option<RootState>>,
    peer: Mutex<Option<Peer<RoleServer>>>,
    /// Start-up archiving, decided once (8.5).
    startup: tokio::sync::OnceCell<()>,
    /// Bumped whenever the state machine settles something, so a waiting call wakes at once.
    changed: tokio::sync::watch::Sender<u64>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Sends `roots/list`; the URIs, or `None` for an error reply. Roots are being deprecated in the
/// MCP specification (SEP-2577), so rmcp marks the call and its fields deprecated; the server
/// still asks while the four hosts answer.
#[allow(deprecated)]
async fn ask_roots(peer: &Peer<RoleServer>) -> Option<Vec<String>> {
    let reply = peer.list_roots().await.ok()?;
    Some(reply.roots.into_iter().map(|r| r.uri).collect())
}

/// A lock that another process holds, wherever the error was wrapped.
fn is_busy(e: &Error) -> bool {
    match e {
        Error::LockTimeout { .. } => true,
        Error::ListFile { source, .. } => is_busy(source),
        _ => false,
    }
}

fn one_line(e: &Error) -> String {
    e.to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tool_error(text: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(text)])
}

impl Shared {
    /// D as the table of 7.1 gives it, resolved now: the working directory is read on each call.
    fn store_for(&self, root: Option<PathBuf>) -> EResult<Store> {
        let mut env = self.env.clone();
        env.cwd = std::env::current_dir().ok();
        let d = start_dir(
            Entry::ServerCall,
            &env,
            None,
            root.as_deref(),
            None,
            self.host,
        )?;
        locate(&d.path, &env)
    }

    /// Does what the state machine asked for after an event.
    fn perform(self: &Arc<Self>, action: Action) {
        let Action::SendRequest(seq) = action else {
            return;
        };
        let deadline = lock(&self.roots).as_ref().and_then(|s| s.next_deadline());
        let peer = lock(&self.peer).clone();
        if let Some(peer) = peer {
            let this = self.clone();
            tokio::spawn(async move {
                let roots = ask_roots(&peer).await;
                let (next, settlement) = {
                    let mut g = lock(&this.roots);
                    match g.as_mut() {
                        Some(st) => (
                            st.on_reply(seq, roots, Instant::now()),
                            st.first_settlement(),
                        ),
                        None => (Action::None, None),
                    }
                };
                this.changed.send_modify(|n| *n += 1);
                this.perform(next);
                if settlement == Some(Settlement::Answered) {
                    this.ensure_startup().await;
                }
            });
        }
        if let Some(deadline) = deadline {
            let this = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(deadline.saturating_duration_since(Instant::now())).await;
                let next = lock(&this.roots)
                    .as_mut()
                    .map_or(Action::None, |st| st.on_tick(Instant::now()));
                this.perform(next);
            });
        }
    }

    /// Start-up archiving (8.5): decided once, when the question of a root is first settled by
    /// the client's answer or by its declaring none; never after a timeout. Every caller waits
    /// for the decision.
    async fn ensure_startup(self: &Arc<Self>) {
        self.startup
            .get_or_init(|| async {
                let (settlement, root) = {
                    let g = lock(&self.roots);
                    match g.as_ref() {
                        Some(st) => (st.first_settlement(), st.current_root().map(PathBuf::from)),
                        None => (None, None),
                    }
                };
                if !matches!(
                    settlement,
                    Some(Settlement::Answered) | Some(Settlement::NoRoots)
                ) {
                    return;
                }
                let this = self.clone();
                let done = tokio::task::spawn_blocking(move || {
                    let store = this.store_for(root)?;
                    archive::run_startup(&store, &this.tuning, &SystemClock)
                })
                .await;
                match done {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) if is_busy(&e) => {
                        eprintln!("repo-tasks: start-up archiving skipped: the list was in use")
                    }
                    Ok(Err(e)) => {
                        eprintln!("repo-tasks: start-up archiving failed: {}", one_line(&e))
                    }
                    Err(e) => eprintln!("repo-tasks: start-up archiving failed: {e}"),
                }
            })
            .await;
    }

    /// The start of a tool call: wait for the roots question as 7.1 says, resolve D once and
    /// hold it, and wait for the start-up decision.
    async fn begin(self: &Arc<Self>) -> std::result::Result<Store, Fail> {
        let root = loop {
            // Subscribed before the state is read, so a settlement in between is not missed.
            let mut changed = self.changed.subscribe();
            let (gate, action, root) = {
                let mut g = lock(&self.roots);
                let Some(st) = g.as_mut() else {
                    return Err(Fail::NotInitialized);
                };
                let (gate, action) = st.call_started(Instant::now());
                (gate, action, st.current_root().map(PathBuf::from))
            };
            self.perform(action);
            match gate {
                CallGate::Refuse => return Err(Fail::NotInitialized),
                CallGate::Go => break root,
                CallGate::Wait(d) => {
                    // At most the time left; a reply wakes the call at once.
                    tokio::select! {
                        _ = tokio::time::sleep(d) => {}
                        _ = changed.changed() => {}
                    }
                    let next = lock(&self.roots)
                        .as_mut()
                        .map_or(Action::None, |st| st.on_tick(Instant::now()));
                    self.perform(next);
                }
            }
        };
        let store = self.store_for(root).map_err(Fail::Tool)?;
        self.ensure_startup().await;
        Ok(store)
    }
}

/// The MCP server of repo-tasks.
#[derive(Clone)]
pub struct RepoTasksServer {
    shared: Arc<Shared>,
    // `#[tool_handler(router = self.tool_router)]` reads it on every call.
    tool_router: ToolRouter<Self>,
}

impl RepoTasksServer {
    pub fn new(host: Host, version: &str, env: Env, opts: ServeOptions) -> RepoTasksServer {
        RepoTasksServer {
            shared: Arc::new(Shared {
                host,
                version: version.to_string(),
                env,
                tuning: Tuning::default(),
                window: opts.roots_window,
                roots: Mutex::new(None),
                peer: Mutex::new(None),
                startup: tokio::sync::OnceCell::new(),
                changed: tokio::sync::watch::channel(0).0,
            }),
            tool_router: Self::tool_router(),
        }
    }

    /// Runs one tool body on the call's store, off the async threads (the engine blocks on files).
    async fn with_store<F>(&self, body: F) -> std::result::Result<CallToolResult, ErrorData>
    where
        F: FnOnce(&Store, &Tuning, Host) -> EResult<String> + Send + 'static,
    {
        let store = match self.shared.begin().await {
            Ok(s) => s,
            Err(Fail::NotInitialized) => return Err(ErrorData::invalid_request(
                "the server is not initialized: send notifications/initialized before a tool call",
                None,
            )),
            Err(Fail::Tool(e)) => return Ok(tool_error(one_line(&e))),
        };
        let shared = self.shared.clone();
        let done =
            tokio::task::spawn_blocking(move || body(&store, &shared.tuning, shared.host)).await;
        Ok(match done {
            Ok(Ok(text)) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Ok(Err(e)) => tool_error(one_line(&e)),
            Err(e) => tool_error(format!("the tool stopped: {e}")),
        })
    }
}

#[rmcp::tool_router]
impl RepoTasksServer {
    #[rmcp::tool(
        description = "Create one to 50 tasks in this repository's shared task list; later tasks may name earlier ones of the same call in blocked_by_earlier. Replies one line per task. Use these tools for work that must outlive the session or be seen by another agent."
    )]
    async fn task_create(
        &self,
        Parameters(args): Parameters<CreateArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        self.with_store(move |s, t, h| ops::create(s, t, h, &SystemClock, args))
            .await
    }

    #[rmcp::tool(
        description = "Read one task by its integer id: status, owner, blockers, what it blocks, times and description; a task that has left the list is answered from the archive. Use these tools for work that must outlive the session or be seen by another agent."
    )]
    async fn task_get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        self.with_store(move |s, t, _| ops::get(s, t, args)).await
    }

    #[rmcp::tool(
        description = "Change a task by its integer id: status, subject, description, owner, blockers, or delete: true for a task that will not be done. Any agent may update any task. Use these tools for work that must outlive the session or be seen by another agent."
    )]
    async fn task_update(
        &self,
        Parameters(args): Parameters<UpdateArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        self.with_store(move |s, t, h| ops::update(s, t, h, &SystemClock, args))
            .await
    }

    #[rmcp::tool(
        description = "List every task in this repository's shared task list, one line each, under a first line naming the list. Use these tools for work that must outlive the session or be seen by another agent."
    )]
    async fn task_list(&self) -> std::result::Result<CallToolResult, ErrorData> {
        self.with_store(|s, t, _| ops::list(s, t, &ListHeader::for_store(s)))
            .await
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for RepoTasksServer {
    /// Stops at 2025-11-25, the newest revision with the `initialize` handshake. rmcp's default
    /// also offers 2026-07-28, on which a client need not initialize and server-to-client requests
    /// (`roots/list`) are forbidden (rmcp 3.10.0 `src/service/server.rs:62-67`); this server
    /// refuses every call before `initialized` and asks for the roots.
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(ProtocolVersion::known_up_to(&MAX_PROTOCOL))
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("repo-tasks", self.shared.version.clone()),
        )
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let declared = context
            .peer
            .peer_info()
            .is_some_and(|i| i.capabilities.roots.is_some());
        // Everything up to the state change is synchronous: a call that follows the
        // notification on the wire must find the server initialized.
        *lock(&self.shared.peer) = Some(context.peer.clone());
        let (action, settlement) = {
            let mut g = lock(&self.shared.roots);
            if g.is_some() {
                return;
            }
            let mut st = RootState::new(declared, self.shared.window);
            let action = st.on_initialized(Instant::now());
            let settlement = st.first_settlement();
            *g = Some(st);
            (action, settlement)
        };
        self.shared.perform(action);
        if settlement == Some(Settlement::NoRoots) {
            self.shared.ensure_startup().await;
        }
    }

    async fn on_roots_list_changed(&self, _context: NotificationContext<RoleServer>) {
        let action = lock(&self.shared.roots)
            .as_mut()
            .map_or(Action::None, |st| st.on_list_changed(Instant::now()));
        self.shared.perform(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_match_the_router() {
        let router = RepoTasksServer::tool_router();
        let listed = router.list_all();
        let table = tool_descriptions();
        assert_eq!(listed.len(), table.len());
        for (name, desc) in table {
            let t = listed.iter().find(|t| t.name == name).expect(name);
            assert_eq!(t.description.as_deref(), Some(desc), "{name}");
            assert!(desc.contains(
                "Use these tools for work that must outlive the session or be seen by another agent."
            ));
        }
    }
}
