//! `skill_asset::render` and `emit-skill <name> --host <h>`: the eight rendered copies (two skills, four hosts).

use opencode_ctl::cli::{dispatch, emit_skill};
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::skill_asset::{render, SkillName, DOCTOR_SKILL, DRIVER_SKILL};

const HOSTS: [Host; 4] = [Host::Claude, Host::Qwen, Host::Agy, Host::Opencode];
const NAMES: [SkillName; 2] = [SkillName::Driver, SkillName::Doctor];

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn skill_str(n: SkillName) -> &'static str {
    match n {
        SkillName::Driver => "opencode-ctl-driver",
        SkillName::Doctor => "opencode-ctl-doctor",
    }
}

fn host_arg(h: Host) -> &'static str {
    match h {
        Host::Claude => "cc",
        Host::Qwen => "qwen",
        Host::Agy => "agy",
        Host::Opencode => "opencode",
        Host::Unknown => unreachable!(),
    }
}

/// The part of a copy that carries the invocation form: the driver through its Related line, the doctor
/// through its front-matter description.
fn own_form_region(n: SkillName, text: &str) -> String {
    match n {
        SkillName::Driver => text
            .lines()
            .skip_while(|l| *l != "# Related")
            .collect::<Vec<_>>()
            .join("\n"),
        SkillName::Doctor => text
            .lines()
            .find(|l| l.starts_with("description:"))
            .unwrap()
            .to_string(),
    }
}

/// Minimal front-matter reader: the block between the first two `---` fences, split at the first `: `.
fn front_matter(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "output must start with ---");
    let mut out = Vec::new();
    let mut closed = false;
    for l in lines {
        if l == "---" {
            closed = true;
            break;
        }
        let (k, v) = l
            .split_once(": ")
            .or_else(|| l.split_once(':'))
            .unwrap_or_else(|| panic!("not a key line: {l:?}"));
        out.push((k.to_string(), v.to_string()));
    }
    assert!(closed, "front matter has no closing fence");
    out
}

fn get<'a>(fm: &'a [(String, String)], k: &str) -> &'a str {
    fm.iter()
        .find(|(key, _)| key == k)
        .unwrap_or_else(|| panic!("front matter key {k} missing"))
        .1
        .as_str()
}

#[test]
fn skill_emit_each_copy_has_own_form_only() {
    let mut count = 0;
    for host in HOSTS {
        for name in NAMES {
            let text = render(name, host).unwrap();
            count += 1;
            assert!(
                text.starts_with("---"),
                "{host:?} {name:?}: starts with front matter"
            );
            assert!(!text.contains("{{"), "{host:?} {name:?}: leftover token");
            assert!(
                !text.contains("SPDX"),
                "{host:?} {name:?}: header line left"
            );
            assert!(
                !text.contains("<!--") && !text.contains("-->"),
                "{host:?} {name:?}: header left"
            );
            let own = host.slash_form(skill_str(name));
            assert!(
                own_form_region(name, &text).contains(&own),
                "{host:?} {name:?}: own form {own} missing"
            );
            for other in HOSTS.into_iter().filter(|o| *o != host) {
                let prefix = format!("/opencode-ctl-mcp-{}:", host_arg(other));
                assert!(
                    !text.contains(&prefix),
                    "{host:?} {name:?} contains {other:?}'s form {prefix}"
                );
            }
            // `--host` as an instruction to run the binary belongs to the OpenCode doctor copy only.
            let flagged = text.contains("doctor-json --host opencode");
            assert_eq!(
                flagged,
                host == Host::Opencode && name == SkillName::Doctor,
                "{host:?} {name:?}: doctor-json --host opencode"
            );
            if host != Host::Opencode {
                assert!(!text.contains("--host opencode"), "{host:?} {name:?}");
            }
            if host == Host::Opencode && name == SkillName::Doctor {
                assert!(!text.contains("install-launcher"));
                assert!(!text.contains("(argument install)"));
                assert!(!text.contains("Installing opencode-pair"));
            }
        }
    }
    assert_eq!(count, 8);
}

#[test]
fn skill_emit_qwen_doctor_argument_free() {
    let text = render(SkillName::Doctor, Host::Qwen).unwrap();
    let fm = front_matter(&text);
    assert_eq!(get(&fm, "argument-hint"), "\"\"");
    assert!(!text.contains("(argument install)"));
    assert!(!text.contains("[install]"));
    let bin = "~/.qwen/extensions/opencode-ctl-mcp-qwen/bin/opencode-ctl-qwen-mcp";
    assert!(text.contains(&format!(
        "hen invoked with no argument and `{bin} doctor-json` reports `launcher_link: missing`, run `{bin} install-launcher`"
    )));
    assert!(text.contains("# Installing opencode-pair"));
}

#[test]
fn skill_emit_install_sections_and_binaries_per_host() {
    for (host, bin) in [
        (
            Host::Claude,
            "${CLAUDE_PLUGIN_ROOT}/bin/opencode-ctl-cc-mcp",
        ),
        (Host::Agy, "opencode-ctl-agy-mcp"),
    ] {
        let text = render(SkillName::Doctor, host).unwrap();
        assert!(text.contains(&format!(
            "hen invoked with `install`, run `{bin} install-launcher` and print its PATH hint"
        )));
        assert!(text.contains(&format!("Run `{bin} doctor-json`, and append")));
        let fm = front_matter(&text);
        assert_eq!(get(&fm, "argument-hint"), "\"[install] [verbose]\"");
        assert!(get(&fm, "description").ends_with(
            "when opencode-pair is missing or stale, or when the user wants opencode-pair installed (argument install)"
        ));
    }
    let oc = render(SkillName::Doctor, Host::Opencode).unwrap();
    assert!(oc.contains(
        "Run `the first element of mcp.servers.opencode-ctl.command in your opencode.jsonc (an absolute path to opencode-ctl-cc-mcp) doctor-json --host opencode`"
    ));
    assert_eq!(get(&front_matter(&oc), "argument-hint"), "\"[verbose]\"");
    let q = render(SkillName::Doctor, Host::Qwen).unwrap();
    assert!(
        get(&front_matter(&q), "description").ends_with("when opencode-pair is missing or stale")
    );
}

#[test]
fn skill_emit_host_names_in_driver() {
    for (host, name) in [
        (Host::Claude, "Claude Code"),
        (Host::Qwen, "Qwen Code"),
        (Host::Agy, "Antigravity"),
        (Host::Opencode, "OpenCode"),
    ] {
        let text = render(SkillName::Driver, host).unwrap();
        assert!(
            text.contains(&format!("You are running as {name}; this skill")),
            "{host:?}"
        );
        // The driver routes errors to its own host's doctor.
        let doctor = host.slash_form("opencode-ctl-doctor");
        assert!(text.contains(&format!("run `{doctor}`.")), "{host:?}");
    }
}

/// The rendered front matter must be valid YAML for every host, although the template line
/// `allowed-tools: {{ALLOWED_TOOLS}}` is not parseable before rendering. Hand parser, no dependency.
#[test]
fn skill_emit_rendered_front_matter_is_valid() {
    for host in HOSTS {
        for name in NAMES {
            let text = render(name, host).unwrap();
            let fm = front_matter(&text);
            assert_eq!(get(&fm, "name"), skill_str(name));
            assert_eq!(get(&fm, "allowed-tools"), "Bash, Read, Write");
            let desc = get(&fm, "description");
            assert!(!desc.is_empty());
            let expect = match name {
                SkillName::Driver => "Delegate bounded implementation work",
                SkillName::Doctor => "Diagnoses why opencode-ctl-mcp tools fail",
            };
            assert!(desc.starts_with(expect), "{host:?} {name:?}: {desc}");
            let keys: Vec<&str> = fm.iter().map(|(k, _)| k.as_str()).collect();
            let want: &[&str] = match name {
                SkillName::Driver => &["name", "description", "allowed-tools"],
                SkillName::Doctor => &["name", "description", "argument-hint", "allowed-tools"],
            };
            assert_eq!(keys, want, "{host:?} {name:?}");
            // Plain-scalar safety: no `: ` or ` #` inside, no leading indicator character.
            for (k, v) in &fm {
                if v.starts_with('"') {
                    assert!(v.ends_with('"') && v.len() >= 2, "{k}: {v}");
                    assert!(!v[1..v.len() - 1].contains('"'), "{k}: {v}");
                    continue;
                }
                assert!(
                    !v.contains(": ") && !v.contains(" #"),
                    "{host:?} {name:?} {k}: {v}"
                );
                assert!(
                    !v.starts_with(['[', '{', '*', '&', '!', '|', '>', '%', '@', '`', '\'']),
                    "{k}: {v}"
                );
                assert_eq!(v.trim(), v, "{k}: surrounding whitespace");
            }
        }
    }
}

#[test]
fn skill_emit_no_whitespace_drift() {
    for host in HOSTS {
        for name in NAMES {
            let text = render(name, host).unwrap();
            assert!(
                text.ends_with('\n') && !text.ends_with("\n\n"),
                "{host:?} {name:?}: exactly one final newline"
            );
            assert!(!text.contains("\r"), "{host:?} {name:?}");
            for (i, l) in text.lines().enumerate() {
                assert_eq!(
                    l.trim_end(),
                    l,
                    "{host:?} {name:?}: trailing whitespace on line {}",
                    i + 1
                );
            }
            assert!(
                !text.contains("\n\n\n"),
                "{host:?} {name:?}: blank-line run"
            );
        }
    }
}

#[test]
fn skill_emit_templates_are_the_canonical_files() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/skill/");
    assert_eq!(
        DRIVER_SKILL,
        std::fs::read_to_string(format!("{dir}opencode-ctl-driver.md")).unwrap()
    );
    assert_eq!(
        DOCTOR_SKILL,
        std::fs::read_to_string(format!("{dir}opencode-ctl-doctor.md")).unwrap()
    );
}

#[test]
fn skill_emit_unknown_host_is_invalid_argument() {
    for name in NAMES {
        let e = render(name, Host::Unknown).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidArgument);
    }
}

#[test]
fn skill_emit_cli_arguments() {
    for host in HOSTS {
        for name in NAMES {
            let want = render(name, host).unwrap();
            let got = emit_skill(&s(&[skill_str(name), "--host", host_arg(host)])).unwrap();
            assert_eq!(got, want);
            let swapped = emit_skill(&s(&["--host", host_arg(host), skill_str(name)])).unwrap();
            assert_eq!(swapped, want);
        }
    }
    for bad in [
        s(&[]),
        s(&["opencode-ctl-driver"]),
        s(&["--host", "cc"]),
        s(&["driver", "--host", "cc"]),
        s(&["opencode-ctl-driver", "--host", "claude"]),
        s(&["opencode-ctl-driver", "--host"]),
        s(&["opencode-ctl-driver", "--host", "cc", "extra"]),
        s(&["opencode-ctl-driver", "--host", "cc", "--host", "qwen"]),
        s(&["opencode-ctl-driver", "opencode-ctl-doctor", "--host", "cc"]),
    ] {
        let e = emit_skill(&bad).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidArgument, "{bad:?}");
    }
}

#[tokio::test]
async fn skill_emit_dispatch_exit_codes() {
    let ok = dispatch(
        Host::Claude,
        "p",
        "1",
        None,
        s(&["emit-skill", "opencode-ctl-doctor", "--host", "qwen"]),
    )
    .await;
    assert_eq!(ok, 0);
    for bad in [
        s(&["emit-skill"]),
        s(&["emit-skill", "nope", "--host", "cc"]),
        s(&["emit-skill", "opencode-ctl-driver", "--host", "nope"]),
        s(&["emit-skill", "opencode-ctl-driver"]),
    ] {
        assert_eq!(
            dispatch(Host::Claude, "p", "1", None, bad.clone()).await,
            2,
            "{bad:?}"
        );
    }
}
