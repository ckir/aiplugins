use opencode_ctl::host::Host;
use opencode_ctl::settings::{
    ctl_home, env_name, load, load_from_fs, parse_frontmatter, Input, Note, NoteReason,
    PermissionPolicy, Settings, Source,
};
use opencode_ctl::testing::{env_map, temp_home};

fn run(
    host: Host,
    env: &[(&str, &str)],
    user: Option<&str>,
    ws: Option<&str>,
) -> (Settings, Vec<Note>) {
    let f = env_map(env);
    load(Input {
        host,
        env: &f,
        user_file: user,
        workspace_file: ws,
    })
}

fn notes_for<'a>(notes: &'a [Note], key: &str) -> Vec<&'a Note> {
    notes.iter().filter(|n| n.key == key).collect()
}

const PROTECTED_WS: &str = "---\nopencode_bin: /x/oc\nserver_url: http://h:1\ndefault_model: a/b\ndefault_agent: build\npermission_policy: allow\npermissions_locked: true\nwait_max_s: 500\n---\n";

#[test]
fn settings_workspace_cannot_set_protected_keys() {
    let (s, notes) = run(Host::Claude, &[], None, Some(PROTECTED_WS));
    let d = Settings::default();
    assert_eq!(s.opencode_bin, d.opencode_bin);
    assert_eq!(s.server_url, None);
    assert_eq!(s.default_model, None);
    assert_eq!(s.default_agent, None);
    assert_eq!(s.permission_policy, PermissionPolicy::Inherit);
    assert!(!s.permissions_locked);
    assert_eq!(s.wait_max_s, 300);
    for key in [
        "opencode_bin",
        "server_url",
        "default_model",
        "default_agent",
        "permission_policy",
        "permissions_locked",
        "wait_max_s",
    ] {
        let n = notes_for(&notes, key);
        assert_eq!(n.len(), 1, "{key}: {notes:?}");
        assert_eq!(n[0].reason, NoteReason::ProtectedInWorkspace, "{key}");
        assert_eq!(n[0].source, Source::WorkspaceFile, "{key}");
        assert_eq!(s.sources[key], Source::Default, "{key}");
    }
}

#[test]
fn settings_workspace_caps() {
    let ws = "---\noutput_max_bytes: 200000\n---\n";
    let (s, notes) = run(Host::Claude, &[], None, Some(ws));
    assert_eq!(s.output_max_bytes, 65536);
    assert_eq!(
        notes_for(&notes, "output_max_bytes")[0].reason,
        NoteReason::Clamped {
            min: 1024,
            max: 65536
        }
    );
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_OUTPUT_MAX_BYTES", "200000")],
        None,
        None,
    );
    assert_eq!(s.output_max_bytes, 200000);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, _) = run(
        Host::Claude,
        &[("OPENCODE_CTL_OUTPUT_MAX_BYTES", "2000000")],
        None,
        None,
    );
    assert_eq!(s.output_max_bytes, 1048576);
}

#[test]
fn settings_env_beats_user_file() {
    let user = "---\npoll_s: 20\ndefault_model: u/m\n---\n";
    let ws = "---\npoll_s: 30\n---\n";
    let (s, _) = run(
        Host::Claude,
        &[("OPENCODE_CTL_POLL_S", "15")],
        Some(user),
        Some(ws),
    );
    assert_eq!(s.poll_s, 15);
    assert_eq!(s.sources["poll_s"], Source::Env);
    let (s, _) = run(Host::Claude, &[], Some(user), Some(ws));
    assert_eq!(s.poll_s, 20);
    assert_eq!(s.sources["poll_s"], Source::UserFile);
    assert_eq!(s.default_model.as_deref(), Some("u/m"));
    let (s, _) = run(Host::Claude, &[], None, Some(ws));
    assert_eq!(s.poll_s, 30);
    assert_eq!(s.sources["poll_s"], Source::WorkspaceFile);
    let (s, _) = run(Host::Claude, &[], None, None);
    assert_eq!(s.poll_s, 10);
    assert_eq!(s.sources["poll_s"], Source::Default);
}

#[test]
fn settings_lock_is_sticky() {
    let user = "---\npermissions_locked: true\npermission_policy: deny\n---\n";
    let (s, notes) = run(
        Host::Claude,
        &[
            ("OPENCODE_CTL_PERMISSIONS_LOCKED", "false"),
            ("OPENCODE_CTL_PERMISSION_POLICY", "allow"),
        ],
        Some(user),
        None,
    );
    assert!(s.permissions_locked);
    assert_eq!(s.lock_source, Some(Source::UserFile));
    assert_eq!(s.permission_policy, PermissionPolicy::Deny);
    assert_eq!(s.sources["permission_policy"], Source::UserFile);
    let n = notes_for(&notes, "permission_policy");
    assert_eq!(n.len(), 1, "{notes:?}");
    assert_eq!(n[0].source, Source::Env);
    assert_eq!(n[0].reason, NoteReason::OverriddenByStickyLock);
}

#[test]
fn settings_clamps_every_tuning_key() {
    // (key, env name, low, high, min, max, getter)
    type Get = fn(&Settings) -> u64;
    let cases: Vec<(&str, &str, &str, &str, i64, i64, Get)> = vec![
        (
            "wait_timeout_s",
            "OPENCODE_CTL_WAIT_TIMEOUT_S",
            "1",
            "9999",
            5,
            300,
            |s| s.wait_timeout_s,
        ),
        ("poll_s", "OPENCODE_CTL_POLL_S", "1", "999", 2, 60, |s| {
            s.poll_s
        }),
        (
            "stall_s",
            "OPENCODE_CTL_STALL_S",
            "1",
            "9999",
            30,
            600,
            |s| s.stall_s,
        ),
        (
            "max_turn_s",
            "OPENCODE_CTL_MAX_TURN_S",
            "1",
            "99999",
            60,
            7200,
            |s| s.max_turn_s,
        ),
        (
            "call_timeout_s",
            "OPENCODE_CTL_CALL_TIMEOUT_S",
            "1",
            "999",
            5,
            120,
            |s| s.call_timeout_s,
        ),
        (
            "body_max_bytes",
            "OPENCODE_CTL_BODY_MAX_BYTES",
            "10",
            "200000",
            1024,
            100000,
            |s| s.body_max_bytes as u64,
        ),
        (
            "output_max_bytes",
            "OPENCODE_CTL_OUTPUT_MAX_BYTES",
            "10",
            "2000000",
            1024,
            1048576,
            |s| s.output_max_bytes as u64,
        ),
        (
            "wait_max_s",
            "OPENCODE_CTL_WAIT_MAX_S",
            "10",
            "1000",
            30,
            600,
            |s| s.wait_max_s,
        ),
    ];
    for (key, envn, low, high, min, max, get) in &cases {
        for (raw, expect) in [(low, *min), (high, *max)] {
            let (s, notes) = run(Host::Claude, &[(envn, raw)], None, None);
            assert_eq!(get(&s), expect as u64, "{key} {raw}");
            let n = notes_for(&notes, key);
            assert_eq!(n.len(), 1, "{key} {raw}: {notes:?}");
            assert_eq!(
                n[0].reason,
                NoteReason::Clamped {
                    min: *min,
                    max: *max
                },
                "{key}"
            );
            assert_eq!(n[0].source, Source::Env);
        }
    }
    // output_max_bytes from a workspace file: 1024..65536.
    for (raw, expect) in [("10", 1024usize), ("200000", 65536)] {
        let ws = format!("---\noutput_max_bytes: {raw}\n---\n");
        let (s, notes) = run(Host::Claude, &[], None, Some(&ws));
        assert_eq!(s.output_max_bytes, expect);
        let n = notes_for(&notes, "output_max_bytes");
        assert_eq!(n.len(), 1);
        assert_eq!(
            n[0].reason,
            NoteReason::Clamped {
                min: 1024,
                max: 65536
            }
        );
        assert_eq!(n[0].source, Source::WorkspaceFile);
    }
    // wait_timeout_s ceiling follows wait_max_s.
    let (s, notes) = run(
        Host::Claude,
        &[
            ("OPENCODE_CTL_WAIT_MAX_S", "60"),
            ("OPENCODE_CTL_WAIT_TIMEOUT_S", "100"),
        ],
        None,
        None,
    );
    assert_eq!(s.wait_timeout_s, 60);
    assert_eq!(
        notes_for(&notes, "wait_timeout_s")[0].reason,
        NoteReason::Clamped { min: 5, max: 60 }
    );
}

#[test]
fn settings_crlf_frontmatter() {
    let user = "---\r\n# comment\r\npoll_s: 20\r\ndefault_model: \"p/m#v\"\r\nstall_s: '45'\r\n---\r\nbody\r\n";
    let (s, notes) = run(Host::Claude, &[], Some(user), None);
    assert_eq!(s.poll_s, 20);
    assert_eq!(s.default_model.as_deref(), Some("p/m#v"));
    assert_eq!(s.stall_s, 45);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(
        parse_frontmatter("---\r\na: b\r\n---\r\n"),
        vec![("a".to_string(), "b".to_string())]
    );
    assert!(parse_frontmatter("a: b\n").is_empty());
}

#[test]
fn settings_unknown_key_noted_default_stands() {
    let (s, notes) = run(Host::Claude, &[], Some("---\nbogus_key: 5\n---\n"), None);
    assert_eq!(s.poll_s, 10);
    let n = notes_for(&notes, "bogus_key");
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].reason, NoteReason::UnknownKey);
    assert_eq!(n[0].source, Source::UserFile);
    assert_eq!(n[0].value, "5");
}

#[test]
fn pairing_bin_env_exception() {
    let (s, _) = run(
        Host::Claude,
        &[
            ("OPENCODE_CTL_BIN", "/opt/oc"),
            ("OPENCODE_CTL_OPENCODE_BIN", "/wrong"),
            ("OPENCODE_CTL_SERVER", "http://127.0.0.1:4096"),
        ],
        None,
        None,
    );
    assert_eq!(s.opencode_bin, Some("/opt/oc".into()));
    assert_eq!(s.server_url.as_deref(), Some("http://127.0.0.1:4096"));
    let (s, _) = run(
        Host::Claude,
        &[("OPENCODE_CTL_OPENCODE_BIN", "/wrong")],
        None,
        None,
    );
    assert_eq!(s.opencode_bin, None);
    assert_eq!(env_name("opencode_bin"), "OPENCODE_CTL_BIN");
    assert_eq!(env_name("server_url"), "OPENCODE_CTL_SERVER");
    assert_eq!(env_name("poll_s"), "OPENCODE_CTL_POLL_S");
}

#[test]
fn settings_empty_env_value_is_unset() {
    let user = "---\npoll_s: 20\nserver_url: http://x\n---\n";
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_POLL_S", "")],
        Some(user),
        None,
    );
    assert_eq!(s.poll_s, 20);
    assert_eq!(s.sources["poll_s"], Source::UserFile);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_SERVER", "")],
        Some(user),
        None,
    );
    assert_eq!(s.server_url, None);
    assert_eq!(s.sources["server_url"], Source::Env);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, _) = run(
        Host::Claude,
        &[("OPENCODE_CTL_POLL_S", "  ")],
        Some(user),
        None,
    );
    assert_eq!(s.poll_s, 20);
}

#[test]
fn settings_wait_max_clamped_on_opencode_host() {
    let (s, notes) = run(
        Host::Opencode,
        &[("OPENCODE_CTL_WAIT_MAX_S", "600")],
        None,
        None,
    );
    assert_eq!(s.wait_max_s, 300);
    let n = notes_for(&notes, "wait_max_s");
    assert_eq!(n.len(), 1);
    assert!(matches!(n[0].reason, NoteReason::Clamped { .. }));
    let (s, notes) = run(
        Host::Opencode,
        &[("OPENCODE_CTL_WAIT_MAX_S", "100")],
        None,
        None,
    );
    assert_eq!(s.wait_max_s, 100);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_WAIT_MAX_S", "600")],
        None,
        None,
    );
    assert_eq!(s.wait_max_s, 600);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn ctl_home_blank_values_ignored() {
    let f = env_map(&[("HOME", " "), ("USERPROFILE", "/u/p")]);
    assert_eq!(
        ctl_home(&f),
        std::path::PathBuf::from("/u/p").join(".opencode-ctl")
    );
    let f = env_map(&[("OPENCODE_CTL_HOME", "  "), ("HOME", "/h")]);
    assert_eq!(
        ctl_home(&f),
        std::path::PathBuf::from("/h").join(".opencode-ctl")
    );
    let f = env_map(&[("OPENCODE_CTL_HOME", "/relocated"), ("HOME", "/h")]);
    assert_eq!(ctl_home(&f), std::path::PathBuf::from("/relocated"));
}

#[test]
fn load_from_fs_empty_server_overrides_user_file() {
    let home = temp_home();
    std::fs::write(
        home.path().join("settings.local.md"),
        "---\nserver_url: http://x\n---\n",
    )
    .unwrap();
    let cwd = temp_home();
    let ws_dir = cwd.path().join(".claude");
    std::fs::create_dir_all(&ws_dir).unwrap();
    std::fs::write(
        ws_dir.join("opencode-ctl-mcp-cc.local.md"),
        "---\npoll_s: 33\n---\n",
    )
    .unwrap();
    let f = |k: &str| (k == "OPENCODE_CTL_SERVER").then(String::new);
    let (s, notes) = load_from_fs(Host::Claude, home.path(), cwd.path(), &f);
    assert_eq!(s.server_url, None);
    assert_eq!(s.sources["server_url"], Source::Env);
    assert_eq!(s.poll_s, 33);
    assert_eq!(s.sources["poll_s"], Source::WorkspaceFile);
    assert!(notes.is_empty(), "{notes:?}");
    let g = |_: &str| None;
    let (s, _) = load_from_fs(Host::Claude, home.path(), cwd.path(), &g);
    assert_eq!(s.server_url.as_deref(), Some("http://x"));
}

#[test]
fn settings_server_url_userinfo_ignored() {
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_SERVER", "http://user:pass@127.0.0.1:4096")],
        None,
        None,
    );
    assert_eq!(s.server_url, None);
    let n = notes_for(&notes, "server_url");
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].reason, NoteReason::UserinfoInUrl);
    assert!(!n[0].value.contains("pass"), "secret leaked: {:?}", n[0]);
}

#[test]
fn settings_default_wait_timeout_follows_lowered_wait_max() {
    let (s, notes) = run(
        Host::Claude,
        &[("OPENCODE_CTL_WAIT_MAX_S", "60")],
        None,
        None,
    );
    assert_eq!(s.wait_timeout_s, 60);
    assert_eq!(s.sources["wait_timeout_s"], Source::Default);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, notes) = run(Host::Claude, &[], None, None);
    assert_eq!(s.wait_timeout_s, 120);
    assert!(notes.is_empty(), "{notes:?}");
    let (s, notes) = run(
        Host::Claude,
        &[
            ("OPENCODE_CTL_WAIT_MAX_S", "60"),
            ("OPENCODE_CTL_WAIT_TIMEOUT_S", "200"),
        ],
        None,
        None,
    );
    assert_eq!(s.wait_timeout_s, 60);
    let n = notes_for(&notes, "wait_timeout_s");
    assert_eq!(n.len(), 1, "{notes:?}");
    assert_eq!(n[0].reason, NoteReason::Clamped { min: 5, max: 60 });
}
