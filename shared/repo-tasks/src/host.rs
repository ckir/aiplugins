// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Host identity: which agent fronts the engine.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Claude,
    Qwen,
    Agy,
    Opencode,
}

/// Narrow the compiled-in host constant. Either `host_arg` or `env_host` being
/// `Some("opencode")` selects `Opencode`; any other `host_arg` is rejected;
/// other env values are ignored.
pub fn resolve(constant: Host, host_arg: Option<&str>, env_host: Option<&str>) -> Result<Host> {
    match host_arg {
        Some("opencode") => return Ok(Host::Opencode),
        Some(other) => {
            return Err(Error::BadHost {
                value: other.to_string(),
            })
        }
        None => {}
    }
    if env_host == Some("opencode") {
        return Ok(Host::Opencode);
    }
    Ok(constant)
}

impl Host {
    /// The value written to a task's `updated_by`.
    pub fn updated_by(&self) -> &'static str {
        match self {
            Host::Claude => "claude-code",
            Host::Qwen => "qwen",
            Host::Agy => "antigravity",
            Host::Opencode => "opencode",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updated_by_names() {
        assert_eq!(Host::Claude.updated_by(), "claude-code");
        assert_eq!(Host::Qwen.updated_by(), "qwen");
        assert_eq!(Host::Agy.updated_by(), "antigravity");
        assert_eq!(Host::Opencode.updated_by(), "opencode");
    }

    #[test]
    fn resolve_narrows_to_opencode_only() {
        assert_eq!(resolve(Host::Claude, None, None).unwrap(), Host::Claude);
        assert_eq!(resolve(Host::Qwen, None, Some("x")).unwrap(), Host::Qwen);
        assert_eq!(
            resolve(Host::Claude, Some("opencode"), None).unwrap(),
            Host::Opencode
        );
        assert_eq!(
            resolve(Host::Agy, None, Some("opencode")).unwrap(),
            Host::Opencode
        );
        let err = resolve(Host::Claude, Some("qwen"), None).unwrap_err();
        assert!(err.to_string().contains("qwen"));
    }
}
