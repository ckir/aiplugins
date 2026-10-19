//! Host identity: which agent fronts the engine and what it may do.

use crate::error::{ErrorCode, ToolError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Claude,
    Qwen,
    Agy,
    Opencode,
    Unknown,
}

/// Narrow the compiled-in host constant. Either `host_arg` or `env_host` being
/// `Some("opencode")` selects `Opencode`; any other `host_arg` is rejected;
/// other env values are ignored.
pub fn resolve(
    constant: Host,
    host_arg: Option<&str>,
    env_host: Option<&str>,
) -> Result<Host, ToolError> {
    match host_arg {
        Some("opencode") => return Ok(Host::Opencode),
        Some(other) => {
            return Err(ToolError::new(
                ErrorCode::InvalidArgument,
                format!("unknown --host value '{other}' (only 'opencode' is accepted)"),
            ))
        }
        None => {}
    }
    if env_host == Some("opencode") {
        return Ok(Host::Opencode);
    }
    Ok(constant)
}

impl Host {
    pub fn binary_name(self) -> &'static str {
        match self {
            Host::Claude | Host::Opencode | Host::Unknown => "opencode-ctl-cc-mcp",
            Host::Qwen => "opencode-ctl-qwen-mcp",
            Host::Agy => "opencode-ctl-agy-mcp",
        }
    }

    pub fn controller(self) -> Option<&'static str> {
        match self {
            Host::Claude => Some("claude"),
            Host::Qwen => Some("qwen"),
            Host::Agy => Some("agy"),
            Host::Opencode | Host::Unknown => None,
        }
    }

    pub fn workspace_settings_rel(self) -> Option<&'static str> {
        match self {
            Host::Claude => Some(".claude/opencode-ctl-mcp-cc.local.md"),
            Host::Opencode => Some(".opencode/opencode-ctl-mcp-opencode.local.md"),
            _ => None,
        }
    }

    pub fn refuses_lifecycle(self) -> bool {
        matches!(self, Host::Opencode | Host::Unknown)
    }

    pub fn offers_launcher(self) -> bool {
        !matches!(self, Host::Opencode)
    }

    pub fn slash_form(self, skill: &str) -> String {
        let suffix = match self {
            Host::Claude | Host::Unknown => "cc",
            Host::Qwen => "qwen",
            Host::Agy => "agy",
            Host::Opencode => "opencode",
        };
        format!("/opencode-ctl-mcp-{suffix}:{skill}")
    }
}
