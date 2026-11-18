// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The roots state machine and URI reading (spec 7.1), driven with injected instants.

use repo_tasks::roots::{
    first_usable, uri_to_path, Action, CallGate, RootState, Settlement, ROOTS_WINDOW,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_millis(2000);

fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

fn state(declared: bool) -> RootState {
    RootState::new(declared, WINDOW).with_is_dir(Box::new(|p: &Path| {
        p == Path::new("/a") || p == Path::new("/b")
    }))
}

fn uris(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn root(s: &RootState) -> Option<PathBuf> {
    s.current_root().map(Path::to_path_buf)
}

#[test]
fn default_window_is_two_seconds() {
    assert_eq!(ROOTS_WINDOW, Duration::from_secs(2));
}

#[test]
fn late_first_reply_accepted_before_any_call_ignored_after() {
    let t0 = Instant::now();
    // No call has resolved D: the late reply is used.
    let mut s = state(true);
    assert_eq!(s.on_initialized(t0), Action::SendRequest);
    assert_eq!(s.on_tick(ms(t0, 2500)), Action::None);
    assert_eq!(s.first_settlement(), Some(Settlement::TimedOut));
    assert_eq!(s.on_reply(uris(&["file:///a"]), ms(t0, 3000)), Action::None);
    assert_eq!(root(&s), Some(PathBuf::from("/a")));

    // A call resolved D after the timeout: the late reply is ignored.
    let mut s = state(true);
    s.on_initialized(t0);
    assert_eq!(s.call_started(ms(t0, 2500)), CallGate::Go);
    assert_eq!(s.on_reply(uris(&["file:///a"]), ms(t0, 3000)), Action::None);
    assert_eq!(root(&s), None);
    assert_eq!(s.first_settlement(), Some(Settlement::TimedOut));
}

#[test]
fn refresh_reply_always_used() {
    let t0 = Instant::now();
    let mut s = state(true);
    s.on_initialized(t0);
    s.on_reply(uris(&["file:///a"]), ms(t0, 100));
    assert_eq!(s.call_started(ms(t0, 200)), CallGate::Go);
    assert_eq!(s.on_list_changed(ms(t0, 300)), Action::SendRequest);
    s.on_reply(uris(&["file:///b"]), ms(t0, 400));
    assert_eq!(root(&s), Some(PathBuf::from("/b")));

    // A refresh that timed out, answered late after a call: still used.
    assert_eq!(s.on_list_changed(ms(t0, 500)), Action::SendRequest);
    assert_eq!(s.call_started(ms(t0, 3000)), CallGate::Go);
    s.on_reply(uris(&["file:///a"]), ms(t0, 3500));
    assert_eq!(root(&s), Some(PathBuf::from("/a")));
}

#[test]
fn notification_during_request_gives_exactly_one_followup_after_reply_and_after_timeout() {
    let t0 = Instant::now();
    // After a reply.
    let mut s = state(true);
    assert_eq!(s.on_initialized(t0), Action::SendRequest);
    assert_eq!(s.on_list_changed(ms(t0, 10)), Action::None);
    assert_eq!(s.on_list_changed(ms(t0, 20)), Action::None);
    assert_eq!(
        s.on_reply(uris(&["file:///a"]), ms(t0, 30)),
        Action::SendRequest
    );
    assert_eq!(s.on_reply(uris(&["file:///b"]), ms(t0, 40)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 5000)), Action::None);
    assert_eq!(root(&s), Some(PathBuf::from("/b")));

    // After a timeout.
    let mut s = state(true);
    s.on_initialized(t0);
    assert_eq!(s.on_list_changed(ms(t0, 10)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 1999)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 2000)), Action::SendRequest);
    assert_eq!(s.on_tick(ms(t0, 2001)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 3999)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 4001)), Action::None);
}

#[test]
fn never_two_requests_in_flight() {
    let t0 = Instant::now();
    let mut s = state(true);
    let sent = std::cell::Cell::new(0);
    let count = |a: Action| {
        if a == Action::SendRequest {
            sent.set(sent.get() + 1);
        }
    };
    count(s.on_initialized(t0));
    for i in 1..5 {
        count(s.on_list_changed(ms(t0, i * 10)));
        count(s.on_tick(ms(t0, i * 10)));
    }
    assert_eq!(sent.get(), 1);
    count(s.on_tick(ms(t0, 2000)));
    assert_eq!(sent.get(), 2);
    // The follow-up is itself in flight: a new notification does not send.
    count(s.on_list_changed(ms(t0, 2100)));
    assert_eq!(sent.get(), 2);
    count(s.on_tick(ms(t0, 3000)));
    assert_eq!(sent.get(), 2);
    // Its timeout settles it and the remembered notification sends the next.
    count(s.on_tick(ms(t0, 4000)));
    assert_eq!(sent.get(), 3);
}

#[test]
fn call_before_initialized_refused() {
    let t0 = Instant::now();
    let mut s = state(true);
    assert_eq!(s.call_started(t0), CallGate::Refuse);
    assert_eq!(s.on_list_changed(t0), Action::None);
    assert_eq!(s.on_tick(ms(t0, 9000)), Action::None);
    assert_eq!(s.call_started(ms(t0, 9000)), CallGate::Refuse);
    assert_eq!(s.first_settlement(), None);
    s.on_initialized(ms(t0, 9001));
    assert_eq!(
        s.call_started(ms(t0, 9002)),
        CallGate::Wait(Duration::from_millis(1999))
    );
}

#[test]
fn calls_wait_only_for_what_is_left_and_never_again_once_settled() {
    let t0 = Instant::now();
    let mut s = state(true);
    s.on_initialized(t0);
    assert_eq!(
        s.call_started(ms(t0, 500)),
        CallGate::Wait(Duration::from_millis(1500))
    );
    assert_eq!(
        s.call_started(ms(t0, 1999)),
        CallGate::Wait(Duration::from_millis(1))
    );
    s.on_reply(uris(&["file:///a"]), ms(t0, 1999));
    assert_eq!(s.call_started(ms(t0, 1999)), CallGate::Go);
    assert_eq!(s.call_started(ms(t0, 9999)), CallGate::Go);
}

#[test]
fn no_roots_declared_never_sends() {
    let t0 = Instant::now();
    let mut s = state(false);
    assert_eq!(s.on_initialized(t0), Action::None);
    assert_eq!(s.on_list_changed(ms(t0, 1)), Action::None);
    assert_eq!(s.on_tick(ms(t0, 5000)), Action::None);
    assert_eq!(s.call_started(ms(t0, 5001)), CallGate::Go);
    assert_eq!(s.on_reply(uris(&["file:///a"]), ms(t0, 5002)), Action::None);
    assert_eq!(root(&s), None);
    assert_eq!(s.first_settlement(), Some(Settlement::NoRoots));
}

#[test]
fn timeout_alone_is_not_answered_for_archiving() {
    let t0 = Instant::now();
    let mut s = state(true);
    s.on_initialized(t0);
    assert_eq!(s.first_settlement(), None);
    s.on_tick(ms(t0, 2000));
    assert_eq!(s.first_settlement(), Some(Settlement::TimedOut));
    s.on_reply(uris(&["file:///a"]), ms(t0, 2500));
    assert_eq!(s.first_settlement(), Some(Settlement::TimedOut));

    // Answered, whether the reply is usable or not.
    let mut s = state(true);
    s.on_initialized(t0);
    s.on_reply(None, ms(t0, 10));
    assert_eq!(s.first_settlement(), Some(Settlement::Answered));
    let mut s = state(true);
    s.on_initialized(t0);
    s.on_reply(uris(&["file:///a"]), ms(t0, 10));
    assert_eq!(s.first_settlement(), Some(Settlement::Answered));
}

#[test]
fn failed_refresh_keeps_held_root() {
    let t0 = Instant::now();
    let mut s = state(true);
    s.on_initialized(t0);
    s.on_reply(uris(&["file:///a"]), ms(t0, 10));
    s.call_started(ms(t0, 20));

    s.on_list_changed(ms(t0, 100));
    s.on_reply(None, ms(t0, 110));
    assert_eq!(root(&s), Some(PathBuf::from("/a")));

    s.on_list_changed(ms(t0, 200));
    s.on_reply(uris(&["http://x/y", "file:///nope"]), ms(t0, 210));
    assert_eq!(root(&s), Some(PathBuf::from("/a")));

    s.on_list_changed(ms(t0, 300));
    s.on_tick(ms(t0, 2300));
    assert_eq!(root(&s), Some(PathBuf::from("/a")));
}

#[test]
fn reply_with_no_request_outstanding_is_ignored() {
    let t0 = Instant::now();
    let mut s = state(true);
    s.on_initialized(t0);
    s.on_reply(uris(&["file:///a"]), ms(t0, 10));
    s.on_reply(uris(&["file:///b"]), ms(t0, 20));
    assert_eq!(root(&s), Some(PathBuf::from("/a")));
}

#[test]
fn uri_table() {
    let p = |u: &str, w: bool| uri_to_path(u, w);
    assert_eq!(
        p("file:///home/u/x", false),
        Some(PathBuf::from("/home/u/x"))
    );
    assert_eq!(
        p("file://localhost/home/u", false),
        Some(PathBuf::from("/home/u"))
    );
    assert_eq!(
        p("file://LOCALHOST/home/u", false),
        Some(PathBuf::from("/home/u"))
    );
    assert_eq!(p("file:/home/u", false), Some(PathBuf::from("/home/u")));
    assert_eq!(p("file://server/share/x", false), None);
    assert_eq!(p("file:///a%20b", false), Some(PathBuf::from("/a b")));
    assert_eq!(p("file:///caf%C3%A9", false), Some(PathBuf::from("/café")));
    assert_eq!(p("file:///a%FFb", false), None);
    assert_eq!(p("file:///a%2", false), None);
    assert_eq!(p("file:///a%zz", false), None);
    assert_eq!(p("http://localhost/a", false), None);
    assert_eq!(p("/plain/path", false), None);
    assert_eq!(p("file:relative", false), None);
    assert_eq!(p("", false), None);
    assert_eq!(p("file:///c:/x", true), Some(PathBuf::from(r"c:\x")));
    assert_eq!(p("file:///C:/x", true), Some(PathBuf::from(r"C:\x")));
    assert_eq!(
        p("file:///c%3A/x%20y", true),
        Some(PathBuf::from(r"c:\x y"))
    );
    assert_eq!(p("file:///C:/a/b", true), Some(PathBuf::from(r"C:\a\b")));
    assert_eq!(p("file:///C:", true), Some(PathBuf::from(r"C:\")));
    // Not a drive letter on non-Windows.
    assert_eq!(p("file:///C:/x", false), Some(PathBuf::from("/C:/x")));
}

#[test]
fn first_usable_skips_unusable_and_missing_directories() {
    let is_dir = |p: &Path| p == Path::new("/b");
    let roots: Vec<String> = ["http://x", "file:///a", "file:///b"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        first_usable(&roots, false, &is_dir),
        Some(PathBuf::from("/b"))
    );
    assert_eq!(first_usable(&roots[..2], false, &is_dir), None);
    assert_eq!(first_usable(&[], false, &is_dir), None);
}
