//! `session_wait`: one evaluation of the anchor, the paged marker search and the terminal-state order.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::clock::Clock;
use crate::compact::{compact_form_field, compact_message, compact_permission, Message, ToolsMode};
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::settings::Settings;
use crate::transport::Transport;

use super::SessionWaitArgs;

/// The route maximum page size.
const PAGE_LIMIT: usize = 200;
/// Hard ceiling of `pending_permissions` / `pending_forms` entries.
const PENDING_CEILING: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitState {
    Idle,
    Failed,
    Interrupted,
    Running,
    AwaitingPermission,
    AwaitingInput,
    Stalled,
    BudgetExceeded,
    Timeout,
}

impl WaitState {
    fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Running => "running",
            Self::AwaitingPermission => "awaiting_permission",
            Self::AwaitingInput => "awaiting_input",
            Self::Stalled => "stalled",
            Self::BudgetExceeded => "budget_exceeded",
            Self::Timeout => "timeout",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorSource {
    /// The anchor is a parked inbox item, parked since this time.
    Inbox { parked_since: i64 },
    /// The anchor is a stored message.
    Message,
    /// Neither holds the id: the clock is the newest message's `time.created`.
    Neither { newest_ms: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub id: String,
    pub created_ms: i64,
    pub source: AnchorSource,
}

/// Everything one evaluation reads. `messages` run OLDEST FIRST and are never trimmed at the anchor.
pub struct Snapshot {
    pub messages: Vec<Message>,
    pub inbox: Vec<Value>,
    pub active: bool,
    pub permissions: Vec<Value>,
    pub forms: Vec<Value>,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub state: WaitState,
    /// Present on the three terminal outcomes: the state's name.
    pub outcome: Option<String>,
    /// The failed marker's error, `{name, message}`.
    pub error: Option<Value>,
    /// `Some(None)` on a terminal state with no newer user message.
    pub concurrent_turn: Option<Option<String>>,
    pub turn_s: Option<i64>,
}

#[derive(Debug, Default)]
pub struct PageState {
    pub newest_cursor_previous: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WaitResult {
    pub state: WaitState,
    pub elapsed_s: i64,
    pub turn_s: Option<i64>,
    pub inbox_pending: bool,
    pub tokens: Option<Value>,
    pub last_assistant_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_permissions: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_forms: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_permissions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_forms: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrent_turn: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parked_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair: Option<Value>,
}

fn is_user(m: &Message) -> bool {
    m.r#type == "user"
}

fn is_idle(m: &Message) -> bool {
    m.r#type == "idle"
}

fn item_id(v: &Value) -> Option<&str> {
    v.get("id").and_then(Value::as_str)
}

fn item_created(v: &Value) -> Option<i64> {
    v.pointer("/time/created").and_then(Value::as_i64)
}

fn parked(anchor: &Anchor, snap: &Snapshot) -> bool {
    matches!(anchor.source, AnchorSource::Inbox { .. })
        && snap
            .inbox
            .iter()
            .any(|i| item_id(i) == Some(anchor.id.as_str()))
}

/// Start of the current non-idle stretch as seen at `end`: the oldest user message at or before `end`
/// with no idle marker between it and `end`, or the anchor, whichever is older.
fn turn_start(anchor: &Anchor, snap: &Snapshot, end: i64) -> Option<i64> {
    let oldest_open_user = snap
        .messages
        .iter()
        .filter(|u| is_user(u) && u.created <= end)
        .filter(|u| {
            !snap
                .messages
                .iter()
                .any(|m| is_idle(m) && m.created > u.created && m.created < end)
        })
        .map(|u| u.created)
        .min();
    let has_user = snap.messages.iter().any(is_user);
    if !has_user && !matches!(anchor.source, AnchorSource::Inbox { .. }) {
        return None;
    }
    Some(oldest_open_user.map_or(anchor.created_ms, |u| u.min(anchor.created_ms)))
}

fn seconds(ms: i64) -> i64 {
    (ms / 1000).max(0)
}

/// Rules 3.1 to 3.5 in order, after rule (0) (a parked anchor skips (1) and (4)).
pub fn evaluate(anchor: &Anchor, snap: &Snapshot, settings: &Settings) -> Evaluation {
    let parked = parked(anchor, snap);
    let turn = |end: i64| turn_start(anchor, snap, end).map(|s| seconds(end - s));

    if !parked {
        let marker = snap
            .messages
            .iter()
            .filter(|m| is_idle(m) && m.created > anchor.created_ms)
            .min_by_key(|m| m.created);
        if let Some(marker) = marker {
            let state = match marker.outcome.as_deref() {
                Some("failed") => WaitState::Failed,
                Some("interrupted") => WaitState::Interrupted,
                _ => WaitState::Idle,
            };
            let concurrent = snap
                .messages
                .iter()
                .rev()
                .find(|m| is_user(m) && m.created > marker.created)
                .map(|m| m.id.clone());
            return Evaluation {
                state,
                outcome: Some(state.name().to_string()),
                // `compact_message` has no error field: `message_from_raw` keeps a marker's compacted error in `tools[0]`.
                error: (state == WaitState::Failed)
                    .then(|| marker.tools.first().cloned())
                    .flatten(),
                concurrent_turn: Some(concurrent),
                turn_s: turn(marker.created),
            };
        }
    }

    let turn_s = turn(snap.now_ms);
    let verdict = |state| Evaluation {
        state,
        outcome: None,
        error: None,
        concurrent_turn: None,
        turn_s,
    };
    if !snap.permissions.is_empty() {
        return verdict(WaitState::AwaitingPermission);
    }
    if !snap.forms.is_empty() {
        return verdict(WaitState::AwaitingInput);
    }
    if turn_s.is_some_and(|t| t > settings.max_turn_s as i64) {
        return verdict(WaitState::BudgetExceeded);
    }
    if !parked && !snap.active {
        let newest = snap.messages.iter().max_by_key(|m| m.created);
        let newest_is_marker_after_anchor =
            newest.is_some_and(|m| is_idle(m) && m.created > anchor.created_ms);
        // An empty message list has no newest message: the anchor's own clock (the call start, see `locate`) stands in.
        let clock = match (newest, &anchor.source) {
            (Some(m), _) => Some(m.created),
            (None, AnchorSource::Neither { .. }) => Some(anchor.created_ms),
            (None, _) => None,
        };
        if let Some(clock) = clock {
            if !newest_is_marker_after_anchor
                && snap.now_ms - clock >= settings.stall_s as i64 * 1000
            {
                return verdict(WaitState::Stalled);
            }
        }
    }
    verdict(WaitState::Running)
}

/// The text of the last `text` part of the newest post-anchor assistant row that has one.
fn last_assistant_text(anchor: &Anchor, messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter(|m| m.r#type == "assistant" && m.created > anchor.created_ms)
        .find_map(|m| m.last_text.clone())
}

fn message_from_raw(raw: &Value) -> Message {
    let mut m = compact_message(raw, false, ToolsMode::None);
    if m.r#type == "idle" {
        if let Some(e) = raw.get("error") {
            let name = e.get("name").or_else(|| e.get("type")).cloned();
            m.tools.push(json!({
                "name": name.unwrap_or(Value::Null),
                "message": e.get("message").cloned().unwrap_or(Value::Null),
            }));
        }
    }
    m
}

fn rows(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn cursor_str(cursor: &Option<Value>, key: &str) -> Option<String> {
    cursor
        .as_ref()
        .and_then(|c| c.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn compact_form(raw: &Value) -> Value {
    let mut o = Map::new();
    for k in ["id", "title", "metadata"] {
        if let Some(v) = raw.get(k) {
            o.insert(k.to_string(), v.clone());
        }
    }
    if let Some(fields) = raw.get("fields").and_then(Value::as_array) {
        o.insert(
            "fields".into(),
            Value::Array(fields.iter().map(compact_form_field).collect()),
        );
    }
    Value::Object(o)
}

/// At most `PENDING_CEILING` compacted entries and the count beyond them.
fn ceiling(list: &[Value], compact: fn(&Value) -> Value) -> (Option<Vec<Value>>, Option<usize>) {
    if list.is_empty() {
        return (None, None);
    }
    let kept: Vec<Value> = list.iter().take(PENDING_CEILING).map(compact).collect();
    let more = list.len().saturating_sub(PENDING_CEILING);
    (Some(kept), (more > 0).then_some(more))
}

enum Stop<'a> {
    /// Page until a message with this id is seen.
    Find(&'a str),
    /// Page until the anchor itself or a message not newer than it is seen.
    Anchor(&'a Anchor),
}

impl Stop<'_> {
    /// The anchor timestamp this stop establishes once `hit` matched `m`.
    fn ts(&self, m: &Message) -> i64 {
        match self {
            Stop::Find(_) => m.created,
            Stop::Anchor(a) => a.created_ms,
        }
    }

    fn hit(&self, m: &Message) -> bool {
        match self {
            Stop::Find(id) => m.id == *id,
            Stop::Anchor(a) => m.id == a.id || m.created <= a.created_ms,
        }
    }
}

/// What anchor resolution leaves for the rest of one evaluation.
struct Located {
    anchor: Anchor,
    messages: Vec<Message>,
    inbox: Vec<Value>,
}

impl<T: Transport, C: Clock> Engine<T, C> {
    async fn list_op(
        &self,
        op: &str,
        session: &str,
    ) -> Result<(Value, Option<Value>, Option<Value>), ToolError> {
        self.call_json(
            op,
            vec![("sessionID".to_string(), session.to_string())],
            None,
        )
        .await
    }

    /// The first read of a slice: `order=desc limit=200`, following `cursor.next` until `stop` hits or
    /// the list ends. Returns the messages OLDEST FIRST and whether `stop` hit.
    async fn read_pages(
        &self,
        session: &str,
        stop: Stop<'_>,
        state: &mut PageState,
    ) -> Result<(Vec<Message>, bool), ToolError> {
        let mut all: Vec<Message> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut first = true;
        let mut hit_ts: Option<i64> = None;
        let found = loop {
            let mut params = vec![
                ("sessionID".to_string(), session.to_string()),
                ("limit".to_string(), PAGE_LIMIT.to_string()),
            ];
            match &cursor {
                // The cursor carries its own order; the route rejects `order` beside it.
                Some(c) => params.push(("cursor".into(), c.clone())),
                None => params.push(("order".into(), "desc".into())),
            }
            let (data, cur, _) = self.call_json("session.message.list", params, None).await?;
            if first {
                state.newest_cursor_previous = cursor_str(&cur, "previous");
                first = false;
            }
            let page: Vec<Message> = rows(&data).iter().map(message_from_raw).collect();
            let empty = page.is_empty();
            if hit_ts.is_none() {
                hit_ts = page.iter().find(|m| stop.hit(m)).map(|m| stop.ts(m));
            }
            all.extend(page);
            // After the stop hits, keep paging until an idle marker older than the anchor has been read, so the
            // whole current non-idle stretch (`turn_s`) is in the snapshot even when the turn spans pages.
            let done = hit_ts.is_some_and(|ts| all.iter().any(|m| is_idle(m) && m.created < ts));
            match cursor_str(&cur, "next") {
                Some(next) if !done && !empty => cursor = Some(next),
                _ => break hit_ts.is_some(),
            }
        };
        all.reverse();
        Ok((all, found))
    }

    /// Messages not read yet, OLDEST FIRST. The first read of a slice (`state` empty) pages back to the
    /// anchor; later reads are one native "newer than" read from `cursor.previous` of the newest page seen.
    pub async fn read_messages_since(
        &self,
        session: &str,
        anchor: &Anchor,
        state: &mut PageState,
    ) -> Result<Vec<Message>, ToolError> {
        let Some(mut cursor) = state.newest_cursor_previous.clone() else {
            return Ok(self
                .read_pages(session, Stop::Anchor(anchor), state)
                .await?
                .0);
        };
        let mut out: Vec<Message> = Vec::new();
        loop {
            let params = vec![
                ("sessionID".to_string(), session.to_string()),
                ("limit".to_string(), PAGE_LIMIT.to_string()),
                ("cursor".to_string(), cursor.clone()),
            ];
            let (data, cur, _) = self.call_json("session.message.list", params, None).await?;
            let mut page: Vec<Message> = rows(&data).iter().map(message_from_raw).collect();
            let full = page.len() >= PAGE_LIMIT;
            if page.is_empty() {
                break;
            }
            page.reverse();
            out.extend(page);
            match cursor_str(&cur, "previous") {
                Some(prev) => {
                    state.newest_cursor_previous = Some(prev.clone());
                    if !full {
                        break;
                    }
                    cursor = prev;
                }
                None => break,
            }
        }
        Ok(out)
    }

    /// The anchor of this slice, or `None` when the session has no user message and no `after` was given.
    async fn locate(
        &self,
        session: &str,
        after: Option<&str>,
        call_start: i64,
        state: &mut PageState,
    ) -> Result<Option<Located>, ToolError> {
        let Some(after) = after else {
            let params = vec![
                ("sessionID".to_string(), session.to_string()),
                ("order".to_string(), "desc".to_string()),
                ("type".to_string(), "user".to_string()),
                ("limit".to_string(), "1".to_string()),
            ];
            let (data, _, _) = self.call_json("session.message.list", params, None).await?;
            let Some(newest_user) = rows(&data).first().map(message_from_raw) else {
                return Ok(None);
            };
            let anchor = Anchor {
                id: newest_user.id,
                created_ms: newest_user.created,
                source: AnchorSource::Message,
            };
            let messages = self.read_messages_since(session, &anchor, state).await?;
            let (inbox, _, _) = self.list_op("session.inbox.list", session).await?;
            return Ok(Some(Located {
                anchor,
                messages,
                inbox: rows(&inbox),
            }));
        };

        let (inbox, _, _) = self.list_op("session.inbox.list", session).await?;
        let inbox = rows(&inbox);
        if let Some(created) = inbox
            .iter()
            .find(|i| item_id(i) == Some(after))
            .and_then(item_created)
        {
            let anchor = Anchor {
                id: after.to_string(),
                created_ms: created,
                source: AnchorSource::Inbox {
                    parked_since: created,
                },
            };
            let messages = self.read_messages_since(session, &anchor, state).await?;
            return Ok(Some(Located {
                anchor,
                messages,
                inbox,
            }));
        }

        // Not parked: it is a stored message, or it is nowhere and the newest message's clock stands in.
        let (messages, found) = self.read_pages(session, Stop::Find(after), state).await?;
        let anchor = match messages.iter().find(|m| m.id == after) {
            Some(m) if found => Anchor {
                id: after.to_string(),
                created_ms: m.created,
                source: AnchorSource::Message,
            },
            _ => {
                let newest_ms = messages
                    .iter()
                    .map(|m| m.created)
                    .max()
                    // RULING: an unknown `after` on an EMPTY message list uses the call-start time as its stall
                    // clock, so the wait reaches `stalled` after `stall_s` instead of staying `running` forever.
                    .unwrap_or(call_start);
                Anchor {
                    id: after.to_string(),
                    created_ms: newest_ms,
                    source: AnchorSource::Neither { newest_ms },
                }
            }
        };
        Ok(Some(Located {
            anchor,
            messages,
            inbox,
        }))
    }

    /// Explicit `after` never replaced; else the newest `user` message. A session with no user message and
    /// no `after` has no anchor: `session_wait` answers `idle` before this point, so here it is `INVALID_REQUEST`.
    pub async fn resolve_anchor(
        &self,
        session: &str,
        after: Option<&str>,
    ) -> Result<Anchor, ToolError> {
        let mut state = PageState::default();
        self.locate(session, after, self.clock().now_ms(), &mut state)
            .await?
            .map(|l| l.anchor)
            .ok_or_else(|| {
                ToolError::new(
                    ErrorCode::InvalidRequest,
                    "the session has no user message to anchor on",
                )
            })
    }

    async fn tokens_of(&self, session: &str) -> Option<Value> {
        let (s, _, _) = self.list_op("session.get", session).await.ok()?;
        Some(s.get("tokens").cloned().unwrap_or(Value::Null))
    }

    pub async fn session_wait(&self, a: SessionWaitArgs) -> ToolResult {
        let session = self.resolve_session(a.session_id.as_deref())?;
        let started = self.clock().now_ms();
        let mut state = PageState::default();
        let pair = self
            .env
            .pair
            .is_some()
            .then(|| json!({"tui": "unknown", "claude": "unknown"}));

        let Some(located) = self
            .locate(&session, a.after.as_deref(), started, &mut state)
            .await?
        else {
            let tokens = self.tokens_of(&session).await;
            let result = WaitResult {
                state: WaitState::Idle,
                elapsed_s: seconds(self.clock().now_ms() - started),
                turn_s: None,
                inbox_pending: false,
                tokens: tokens.filter(|t| !t.is_null()),
                last_assistant_text: None,
                pending_permissions: None,
                pending_forms: None,
                more_permissions: None,
                more_forms: None,
                outcome: Some("idle".into()),
                verify_hint: None,
                concurrent_turn: Some(None),
                error: None,
                parked_s: None,
                pair,
            };
            return Ok(serde_json::to_value(result).unwrap_or(Value::Null));
        };
        let Located {
            anchor,
            messages,
            inbox,
        } = located;

        let (permissions, _, _) = self.list_op("session.permission.list", &session).await?;
        let (forms, _, _) = self.list_op("session.form.list", &session).await?;
        let (active, _, _) = self.list_op("session.active", &session).await?;
        let tokens = self.tokens_of(&session).await;

        let snap = Snapshot {
            messages,
            inbox,
            active: active.get(&session).is_some(),
            permissions: rows(&permissions),
            forms: rows(&forms),
            now_ms: self.clock().now_ms(),
        };
        let ev = evaluate(&anchor, &snap, &self.settings);

        let parked_s = if snap.inbox.is_empty() {
            None
        } else {
            let since = match anchor.source {
                AnchorSource::Inbox { parked_since } if parked(&anchor, &snap) => {
                    Some(parked_since)
                }
                _ => snap.inbox.iter().filter_map(item_created).min(),
            };
            since.map(|s| seconds(snap.now_ms - s))
        };
        let (pending_permissions, more_permissions) =
            ceiling(&snap.permissions, compact_permission);
        let (pending_forms, more_forms) = ceiling(&snap.forms, compact_form);
        let result = WaitResult {
            state: ev.state,
            elapsed_s: seconds(snap.now_ms - started),
            turn_s: ev.turn_s,
            inbox_pending: !snap.inbox.is_empty(),
            tokens,
            last_assistant_text: last_assistant_text(&anchor, &snap.messages),
            pending_permissions,
            pending_forms,
            more_permissions,
            more_forms,
            verify_hint: (ev.state == WaitState::Idle).then(|| {
                format!(
                    "call session_changes with from={} to see what this turn changed",
                    anchor.id
                )
            }),
            outcome: ev.outcome,
            concurrent_turn: ev.concurrent_turn,
            error: ev.error,
            parked_s,
            pair,
        };
        Ok(serde_json::to_value(result).unwrap_or(Value::Null))
    }
}
