use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use opencode_ctl::cli::{dispatch, route, version_line, Route};
use opencode_ctl::clock::{Clock, TestClock};
use opencode_ctl::error::{ErrorCode, ToolError};
use opencode_ctl::host::{resolve, Host};
use opencode_ctl::liveness::{parse_kern_boottime, parse_lstart, parse_proc_stat_starttime};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[tokio::test]
async fn version_flag_prints_name_and_version() {
    assert_eq!(
        version_line("opencode-ctl-cc-mcp", "0.1.0"),
        "opencode-ctl-cc-mcp 0.1.0"
    );
    let code = dispatch(
        Host::Claude,
        "opencode-ctl-cc-mcp",
        "0.1.0",
        None,
        s(&["--version"]),
    )
    .await;
    assert_eq!(code, 0);
}

#[tokio::test]
async fn usage_exit_code_is_2() {
    assert_eq!(dispatch(Host::Claude, "p", "1", None, vec![]).await, 2);
    assert_eq!(
        dispatch(Host::Claude, "p", "1", None, s(&["bogus"])).await,
        2
    );
    assert_eq!(
        dispatch(Host::Qwen, "p", "1", Some("x"), s(&["gc"])).await,
        2
    );
}

#[test]
fn host_narrowing_from_each_constant() {
    let constants = [
        Host::Claude,
        Host::Qwen,
        Host::Agy,
        Host::Opencode,
        Host::Unknown,
    ];
    for c in constants {
        for arg in [None, Some("opencode")] {
            for env in [None, Some("opencode")] {
                let got = resolve(c, arg, env).unwrap();
                if arg.is_some() || env.is_some() {
                    assert_eq!(got, Host::Opencode);
                } else {
                    assert_eq!(got, c);
                }
            }
        }
        // other env values are ignored
        assert_eq!(resolve(c, None, Some("qwen")).unwrap(), c);
    }
}

#[test]
fn host_unknown_host_flag_rejected() {
    let e = resolve(Host::Claude, Some("qwen"), None).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
}

#[test]
fn error_codes_round_trip() {
    let expected = [
        "OPENCODE_NOT_FOUND",
        "SERVICE_DOWN",
        "AUTH_FAILED",
        "SESSION_NOT_FOUND",
        "NO_SESSION",
        "PAIR_NOT_FOUND",
        "INVALID_ARGUMENT",
        "INVALID_REQUEST",
        "REFUSED_ON_HOST",
        "CONFIRM_REQUIRED",
        "PERMISSIONS_LOCKED",
        "MUTATION_BLOCKED",
        "BLOCKED_OPERATION",
        "NON_JSON_RESPONSE",
        "UNKNOWN_OPERATION",
        "BODY_TOO_LARGE",
        "CONFLICT",
        "DEADLINE",
        "IO_ERROR",
        "LAUNCHER_CONFLICT",
        "CONTROLLER_NOT_FOUND",
        "CLI_ERROR",
    ];
    let got: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(got, expected);
    assert_eq!(got.iter().collect::<HashSet<_>>().len(), 22);
    for c in ErrorCode::ALL {
        let j = ToolError::new(c, "m").to_json();
        assert_eq!(j["error"]["code"], c.as_str());
    }
    let j = ToolError::new(ErrorCode::Conflict, "m")
        .hint("h")
        .details(serde_json::json!({"a":1}))
        .to_json();
    assert_eq!(j["error"]["hint"], "h");
    assert_eq!(j["error"]["details"]["a"], 1);
    let io: ToolError = std::io::Error::other("x").into();
    assert_eq!(io.code, ErrorCode::IoError);
}

#[tokio::test]
async fn test_clock_sleep_wakes_on_advance() {
    let clock = TestClock::new(0);
    let c2 = clock.clone();
    let h = tokio::spawn(async move { c2.sleep(Duration::from_millis(500)).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(clock.pending_sleepers(), 1);
    assert!(!h.is_finished());
    clock.advance(499);
    tokio::task::yield_now().await;
    assert!(!h.is_finished());
    clock.advance(1);
    h.await.unwrap();
    assert_eq!(clock.pending_sleepers(), 0);
}

#[tokio::test]
async fn test_clock_no_lost_wakeup() {
    for _ in 0..100 {
        let clock = TestClock::new(0);
        let c2 = clock.clone();
        let h = tokio::spawn(async move { c2.sleep(Duration::from_millis(10)).await });
        // one yield lets the spawned task call sleep() (fixing its deadline) and park
        tokio::task::yield_now().await;
        clock.advance(20);
        tokio::time::timeout(Duration::from_secs(5), h)
            .await
            .expect("sleeper hung")
            .unwrap();
    }
}

#[tokio::test]
async fn test_clock_cancelled_sleep_not_counted() {
    let clock = TestClock::new(0);
    tokio::select! {
        _ = clock.sleep(Duration::from_secs(60)) => panic!("must not finish"),
        _ = tokio::task::yield_now() => {}
    }
    assert_eq!(clock.pending_sleepers(), 0);
}

#[test]
fn route_table_basic() {
    assert_eq!(route(None, &s(&["--version"])), Route::Version);
    assert_eq!(route(None, &s(&["-V"])), Route::Version);
    assert_eq!(
        route(None, &s(&["serve"])),
        Route::Sub {
            name: "serve".into(),
            rest: vec![]
        }
    );
    assert_eq!(
        route(None, &s(&["emit-skill", "driver", "--host", "claude"])),
        Route::Sub {
            name: "emit-skill".into(),
            rest: s(&["driver", "--host", "claude"])
        }
    );
    assert_eq!(
        route(Some("opencode-ctl-cc-mcp"), &s(&["pair", "x"])),
        Route::Sub {
            name: "pair".into(),
            rest: s(&["x"])
        }
    );
    assert_eq!(route(None, &s(&["nonsense"])), Route::Usage);
    assert_eq!(route(None, &[]), Route::Usage);
}

#[tokio::test]
async fn test_clock_run_until_idle_advances_to_next_deadline() {
    let clock = TestClock::new(0);
    let log: Arc<std::sync::Mutex<Vec<i64>>> = Arc::default();
    let (c1, l1) = (clock.clone(), log.clone());
    let a = async move {
        c1.sleep(Duration::from_secs(10)).await;
        l1.lock().unwrap().push(c1.now_ms());
        c1.sleep(Duration::from_secs(5)).await;
        l1.lock().unwrap().push(c1.now_ms());
    };
    let (c2, l2) = (clock.clone(), log.clone());
    let b = async move {
        c2.sleep(Duration::from_secs(25)).await;
        l2.lock().unwrap().push(c2.now_ms());
    };
    tokio::select! {
        _ = async { tokio::join!(a, b) } => {}
        _ = clock.run_until_idle() => unreachable!(),
    }
    assert_eq!(*log.lock().unwrap(), vec![10_000, 15_000, 25_000]);
}

#[test]
fn probe_parsers_from_captured_strings() {
    assert_eq!(
        parse_kern_boottime("{ sec = 1760000000, usec = 123 } Sun Oct 18 12:00:00 2026"),
        Some(1_760_000_000)
    );
    assert_eq!(parse_kern_boottime("garbage"), None);
    // 2026-10-18 12:34:56 UTC
    assert_eq!(
        parse_lstart("Sun Oct 18 12:34:56 2026"),
        Some(1_792_326_896)
    );
    assert_eq!(parse_lstart("Thu Jan  1 00:00:00 1970"), Some(0));
    assert_eq!(parse_lstart("nonsense"), None);
    let stat = "1234 (my (weird) prog) S 1 1234 1234 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 12345678 500 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0";
    assert_eq!(parse_proc_stat_starttime(stat), Some("987654".to_string()));
    assert_eq!(parse_proc_stat_starttime("no paren"), None);
}
