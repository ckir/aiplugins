// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The client's roots (spec 7.1): reading a `file:` URI, and the pure state machine that settles
//! `roots/list` requests. Nothing here sleeps or reads a clock: time arrives as `Instant` values,
//! and the only I/O is the `is_dir` check, which a caller may replace.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a `roots/list` request may stay unanswered before it is settled by timeout.
pub const ROOTS_WINDOW: Duration = Duration::from_secs(2);

/// Outstanding requests kept for matching late replies; a client that never replies cannot grow it.
const MAX_OUTSTANDING: usize = 16;

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decodes as UTF-8; a malformed escape or invalid UTF-8 is `None`.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hi = hex(*b.get(i + 1)?)?;
            let lo = hex(*b.get(i + 2)?)?;
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Reads a `file:` URI as RFC 8089 gives it (7.1). `None` means unusable.
pub fn uri_to_path(uri: &str, windows: bool) -> Option<PathBuf> {
    let rest = uri.get(..5).filter(|s| s.eq_ignore_ascii_case("file:"))?;
    let rest = &uri[rest.len()..];
    let path = if let Some(after) = rest.strip_prefix("//") {
        let end = after.find('/').unwrap_or(after.len());
        let (authority, path) = after.split_at(end);
        if !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost")) {
            return None;
        }
        path
    } else {
        rest
    };
    if !path.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(path)?;
    let b = decoded.as_bytes();
    if windows
        && b.len() >= 3
        && b[1].is_ascii_alphabetic()
        && b[2] == b':'
        && (b.len() == 3 || b[3] == b'/')
    {
        let tail = decoded[3..].replace('/', "\\");
        let tail = if tail.is_empty() {
            "\\".to_string()
        } else {
            tail
        };
        return Some(PathBuf::from(format!("{}:{}", b[1] as char, tail)));
    }
    Some(PathBuf::from(decoded))
}

/// The first root, in order, whose URI is usable and whose path is an existing directory.
pub fn first_usable(
    roots: &[String],
    windows: bool,
    is_dir: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    roots
        .iter()
        .filter_map(|u| uri_to_path(u, windows))
        .find(|p| is_dir(p))
}

/// What the caller must do after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// Send one `roots/list` request now, with this sequence number; its reply must be
    /// passed back to `on_reply` with the same number.
    SendRequest(u64),
}

/// What a tool call must do before it resolves D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallGate {
    /// `notifications/initialized` has not arrived: refuse with JSON-RPC -32600.
    Refuse,
    /// A request is unsettled: wait this long (at most), then ask again.
    Wait(Duration),
    /// Resolve D now. Returning this marks D as resolved.
    Go,
}

/// How the first question of a root was settled (what 8.5 consults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    /// The client declared no roots.
    NoRoots,
    /// A reply arrived, usable or not.
    Answered,
    /// Only the window running out settled it.
    TimedOut,
}

#[derive(Debug)]
struct Outstanding {
    seq: u64,
    first: bool,
    /// Settled by timeout; its reply, if any, is still to come.
    settled: bool,
    deadline: Instant,
}

type IsDir = Box<dyn Fn(&Path) -> bool + Send>;

/// The 7.1 state machine. Drive it from one task, behind a mutex.
pub struct RootState {
    declared: bool,
    window: Duration,
    initialized: bool,
    /// Requests in the order sent; only the last can be unsettled.
    outstanding: VecDeque<Outstanding>,
    first_sent: bool,
    next_seq: u64,
    pending_refresh: bool,
    d_resolved: bool,
    root: Option<PathBuf>,
    settlement: Option<Settlement>,
    is_dir: IsDir,
    windows: bool,
}

impl RootState {
    pub fn new(declared_roots: bool, window: Duration) -> RootState {
        RootState {
            declared: declared_roots,
            window,
            initialized: false,
            outstanding: VecDeque::new(),
            first_sent: false,
            next_seq: 0,
            pending_refresh: false,
            d_resolved: false,
            root: None,
            settlement: None,
            is_dir: Box::new(|p: &Path| p.is_dir()),
            windows: cfg!(windows),
        }
    }

    /// Replaces the directory check (default: `Path::is_dir`), for tests.
    pub fn with_is_dir(mut self, is_dir: IsDir) -> RootState {
        self.is_dir = is_dir;
        self
    }

    fn in_flight(&self) -> Option<&Outstanding> {
        self.outstanding.back().filter(|o| !o.settled)
    }

    fn settle_timeouts(&mut self, now: Instant) {
        if let Some(o) = self.outstanding.back_mut() {
            if !o.settled && now >= o.deadline {
                o.settled = true;
                if o.first && self.settlement.is_none() {
                    self.settlement = Some(Settlement::TimedOut);
                }
            }
        }
    }

    fn send_if_due(&mut self, now: Instant) -> Action {
        if !self.initialized || !self.declared || self.in_flight().is_some() {
            return Action::None;
        }
        if self.first_sent && !self.pending_refresh {
            return Action::None;
        }
        self.pending_refresh = false;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.outstanding.push_back(Outstanding {
            seq,
            first: !self.first_sent,
            settled: false,
            deadline: now + self.window,
        });
        self.first_sent = true;
        while self.outstanding.len() > MAX_OUTSTANDING {
            self.outstanding.pop_front();
        }
        Action::SendRequest(seq)
    }

    /// `notifications/initialized` arrived.
    pub fn on_initialized(&mut self, now: Instant) -> Action {
        if self.initialized {
            return Action::None;
        }
        self.initialized = true;
        if !self.declared {
            self.settlement = Some(Settlement::NoRoots);
            return Action::None;
        }
        self.send_if_due(now)
    }

    /// A reply to `roots/list` arrived: the roots' URIs, or `None` for an error reply.
    /// `seq` is the number the request carried; an unknown one is ignored.
    pub fn on_reply(&mut self, seq: u64, roots: Option<Vec<String>>, now: Instant) -> Action {
        self.settle_timeouts(now);
        let Some(idx) = self.outstanding.iter().position(|o| o.seq == seq) else {
            return Action::None;
        };
        let Some(req) = self.outstanding.remove(idx) else {
            return Action::None;
        };
        let use_it = if req.settled {
            !req.first || !self.d_resolved
        } else {
            if req.first && self.settlement.is_none() {
                self.settlement = Some(Settlement::Answered);
            }
            true
        };
        if use_it {
            if let Some(list) = roots {
                if let Some(p) = first_usable(&list, self.windows, &*self.is_dir) {
                    self.root = Some(p);
                }
            }
        }
        self.send_if_due(now)
    }

    /// `notifications/roots/list_changed` arrived.
    pub fn on_list_changed(&mut self, now: Instant) -> Action {
        if !self.initialized || !self.declared {
            return Action::None;
        }
        self.settle_timeouts(now);
        self.pending_refresh = true;
        self.send_if_due(now)
    }

    /// Time has passed: settle by timeout and send a remembered refresh.
    pub fn on_tick(&mut self, now: Instant) -> Action {
        self.settle_timeouts(now);
        self.send_if_due(now)
    }

    /// A tool call starts. `Go` marks D as resolved. The action is what `on_tick` would return:
    /// a remembered refresh to send once this call has settled a timeout.
    pub fn call_started(&mut self, now: Instant) -> (CallGate, Action) {
        if !self.initialized {
            return (CallGate::Refuse, Action::None);
        }
        self.settle_timeouts(now);
        if let Some(o) = self.in_flight() {
            return (
                CallGate::Wait(o.deadline.saturating_duration_since(now)),
                Action::None,
            );
        }
        self.d_resolved = true;
        (CallGate::Go, self.send_if_due(now))
    }

    /// The deadline of the request in flight, so the caller can schedule an `on_tick`.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.in_flight().map(|o| o.deadline)
    }

    pub fn current_root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn first_settlement(&self) -> Option<Settlement> {
        self.settlement
    }
}
