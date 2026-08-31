//! Antigravity-specific glue for the `re-ghidra-mcp-agy` plugin.
//!
//! The server, its 19 tools and the Ghidra worker all live in the shared
//! `ghidra-mcp` crate. What is genuinely Antigravity-shaped lives here: reading
//! `.agents/re-ghidra-mcp-agy.local.md`, and the preflight the `PreInvocation`
//! hook reports.
//!
//! Why a settings file matters more for this plugin than for most: **one server
//! is one Ghidra project.** The project directory, project name and bootstrap
//! program are per-workspace by nature, so without a settings layer every repo
//! needs a hand-written `.mcp.json` that duplicates the plugin's registration
//! just to carry three strings.

use ghidra_mcp::config::RawConfig;
use std::path::{Path, PathBuf};

// ── Settings ───────────────────────────────────────────────────────────

/// The settings-file layer, in the shape the shared config resolver consumes.
pub fn settings_from_markdown(source: &str) -> RawConfig {
    let mut config = RawConfig::default();
    let Some(frontmatter) = extract_frontmatter(source) else {
        return config;
    };

    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
        if value.is_empty() {
            continue;
        }
        let value = Some(value.to_string());
        match key.trim() {
            "ghidra_install_dir" => config.ghidra_install_dir = value,
            "project_dir" => config.project_dir = value,
            "project_name" => config.project_name = value,
            "bootstrap_program" => config.bootstrap_program = value,
            "bootstrap_program_path" => config.bootstrap_program_path = value,
            "max_heap" => config.max_heap = value,
            _ => {}
        }
    }
    config
}

/// The settings file this plugin reads, relative to a project root.
pub fn settings_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join(".agents")
        .join("re-ghidra-mcp-agy.local.md")
}

/// Load the settings layer for `project_dir`. A missing or unreadable file is
/// not an error — it simply contributes nothing, and the environment or CLI
/// flags supply the values instead.
pub fn load_settings(project_dir: &Path) -> RawConfig {
    match std::fs::read_to_string(settings_path(project_dir)) {
        Ok(text) => settings_from_markdown(&text),
        Err(_) => RawConfig::default(),
    }
}

/// Extract the YAML frontmatter block from a `.local.md` file.
fn extract_frontmatter(source: &str) -> Option<&str> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let rest = source.strip_prefix("---")?;
    let rest = rest.strip_prefix('\r').unwrap_or(rest);
    let rest = rest.strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

// ── Preflight ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Probe {
    pub launcher_present: bool,
    pub java_major: Option<u32>,
}

pub const MIN_JAVA_MAJOR: u32 = 21;

pub fn preflight(cfg: &RawConfig, probe: &Probe) -> Vec<String> {
    let mut findings = Vec::new();

    match cfg.ghidra_install_dir.as_deref() {
        None => findings.push(
            "GHIDRA_INSTALL_DIR is not set. Point it at your Ghidra install root \
             (the directory containing `support/`)."
                .to_string(),
        ),
        Some(dir) if !probe.launcher_present => findings.push(format!(
            "GHIDRA_INSTALL_DIR is set to `{dir}` but there is no \
             `support/analyzeHeadless` there, so it is not a Ghidra install root."
        )),
        Some(_) => {}
    }

    match probe.java_major {
        None => findings.push(
            "No `java` on PATH. Ghidra 12.1.2 requires JDK 21 \
             (`application.java.min=21`)."
                .to_string(),
        ),
        Some(v) if v < MIN_JAVA_MAJOR => findings.push(format!(
            "`java` on PATH reports version {v}; Ghidra 12.1.2 requires \
             JDK {MIN_JAVA_MAJOR} or newer."
        )),
        Some(_) => {}
    }

    let missing: Vec<&str> = [
        ("project_dir", cfg.project_dir.is_none()),
        ("project_name", cfg.project_name.is_none()),
        ("bootstrap_program", cfg.bootstrap_program.is_none()),
    ]
    .into_iter()
    .filter_map(|(name, absent)| absent.then_some(name))
    .collect();

    if !missing.is_empty() {
        findings.push(format!(
            "Unset: {}. Set them in `.agents/re-ghidra-mcp-agy.local.md` \
             or as GHIDRA_MCP_PROJECT_DIR / GHIDRA_MCP_PROJECT_NAME / \
             GHIDRA_MCP_BOOTSTRAP_PROGRAM.",
            missing.join(", ")
        ));
    }

    findings
}

pub fn parse_java_major(version_output: &str) -> Option<u32> {
    let quoted = version_output.split('"').nth(1)?;
    let mut parts = quoted.split(['.', '_', '-']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}
