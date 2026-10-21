//! The two opencode-ctl skills, embedded as templates and rendered per host.
//!
//! The canonical templates live INSIDE this crate (`skill/`), so `include_str!` needs no `..` traversal.
//! `render` strips the maintainer header, substitutes the host placeholders and returns the exact bytes
//! `emit-skill <name> --host <h>` prints; the committed per-plugin copies are pinned against this output.

use crate::error::{ErrorCode, ToolError};
use crate::host::Host;

/// The driver skill template, embedded at compile time (version-locked to this binary).
pub const DRIVER_SKILL: &str = include_str!("../skill/opencode-ctl-driver.md");

/// The doctor skill template, embedded at compile time.
pub const DOCTOR_SKILL: &str = include_str!("../skill/opencode-ctl-doctor.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillName {
    Driver,
    Doctor,
}

impl SkillName {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillName::Driver => "opencode-ctl-driver",
            SkillName::Doctor => "opencode-ctl-doctor",
        }
    }

    pub fn parse(s: &str) -> Option<SkillName> {
        match s {
            "opencode-ctl-driver" => Some(SkillName::Driver),
            "opencode-ctl-doctor" => Some(SkillName::Doctor),
            _ => None,
        }
    }

    fn template(self) -> &'static str {
        match self {
            SkillName::Driver => DRIVER_SKILL,
            SkillName::Doctor => DOCTOR_SKILL,
        }
    }
}

/// Remove the leading HTML-comment header (the same rule as `shared/ghidra-mcp/src/skill_asset.rs:20-40`:
/// only when the template starts with `<!--`, resuming after whatever line ending follows the `-->` line).
fn strip_header(t: &str) -> &str {
    if !t.starts_with("<!--") {
        return t;
    }
    match t.find("-->") {
        Some(i) => match t[i..].find('\n') {
            Some(j) => &t[i + j + 1..],
            None => "",
        },
        None => t,
    }
}

fn host_name(host: Host) -> &'static str {
    match host {
        Host::Claude | Host::Unknown => "Claude Code",
        Host::Qwen => "Qwen Code",
        Host::Agy => "Antigravity",
        Host::Opencode => "OpenCode",
    }
}

fn binary(host: Host) -> &'static str {
    match host {
        Host::Claude | Host::Unknown => "${CLAUDE_PLUGIN_ROOT}/bin/opencode-ctl-cc-mcp",
        Host::Qwen => "~/.qwen/extensions/opencode-ctl-mcp-qwen/bin/opencode-ctl-qwen-mcp",
        Host::Agy => "opencode-ctl-agy-mcp",
        Host::Opencode => {
            "the first element of mcp.servers.opencode-ctl.command in your opencode.jsonc (an absolute path to opencode-ctl-cc-mcp)"
        }
    }
}

fn argument_hint(host: Host) -> &'static str {
    match host {
        Host::Claude | Host::Agy | Host::Unknown => "[install] [verbose]",
        Host::Opencode => "[verbose]",
        Host::Qwen => "",
    }
}

fn install_clause(host: Host) -> &'static str {
    match host {
        Host::Claude | Host::Agy | Host::Unknown => {
            ", or when the user wants opencode-pair installed (argument install)"
        }
        Host::Opencode | Host::Qwen => "",
    }
}

/// The whole "Installing opencode-pair" section, without a trailing newline; it still carries `{{BINARY}}`.
fn install_section(host: Host) -> &'static str {
    match host {
        Host::Claude | Host::Agy | Host::Unknown => {
            "# Installing opencode-pair\n\nWhen invoked with `install`, run `{{BINARY}} install-launcher` and print its PATH hint."
        }
        Host::Qwen => {
            "# Installing opencode-pair\n\nWhen invoked with no argument and `{{BINARY}} doctor-json` reports `launcher_link: missing`, run `{{BINARY}} install-launcher`."
        }
        Host::Opencode => "",
    }
}

/// The bytes `emit-skill` prints for `(name, host)`: header stripped, every token substituted, exactly one
/// final newline. A leftover `{{` is an error rather than silently shipped.
pub fn render(name: SkillName, host: Host) -> Result<String, ToolError> {
    if host == Host::Unknown {
        return Err(ToolError::new(
            ErrorCode::InvalidArgument,
            "cannot render a skill for an unknown host",
        ));
    }
    let doctor = host.slash_form(SkillName::Doctor.as_str());
    let hint = argument_hint(host);
    let host_flag = if host == Host::Opencode {
        " --host opencode"
    } else {
        ""
    };
    // INSTALL_SECTION first: its text contains {{BINARY}}.
    let mut out =
        strip_header(name.template()).replace("{{INSTALL_SECTION}}", install_section(host));
    for (token, value) in [
        ("{{INVOCATION}}", host.slash_form(name.as_str())),
        ("{{DOCTOR_INVOCATION}}", doctor),
        ("{{BINARY}}", binary(host).to_string()),
        ("{{ARGUMENT_HINT}}", hint.to_string()),
        ("{{ALLOWED_TOOLS}}", "Bash, Read, Write".to_string()),
        ("{{INSTALL_CLAUSE}}", install_clause(host).to_string()),
        ("{{HOST_FLAG}}", host_flag.to_string()),
        ("{{HOST_NAME}}", host_name(host).to_string()),
    ] {
        out = out.replace(token, &value);
    }
    if out.contains("{{") {
        return Err(ToolError::new(
            ErrorCode::CliError,
            format!(
                "skill template {} has an unsubstituted token",
                name.as_str()
            ),
        ));
    }
    // An empty install section leaves trailing blank lines; every copy ends in exactly one newline.
    let mut out = out.trim_end().to_string();
    out.push('\n');
    Ok(out)
}
