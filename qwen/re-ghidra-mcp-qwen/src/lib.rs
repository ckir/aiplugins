//! Qwen-Code-specific glue for the `re-ghidra-mcp-qwen` extension.
//!
//! The server, its 19 tools and the Ghidra worker all live in the shared
//! `ghidra-mcp` crate. What is genuinely Qwen-Code-shaped lives here: reading
//! `.qwen/re-ghidra-mcp-qwen.local.md`, and the preflight the `SessionStart`
//! hook reports.
//!
//! Why a settings file matters more for this extension than for most: **one server
//! is one Ghidra project.** The project directory, project name and bootstrap
//! program are per-workspace by nature, so without a settings layer every repo
//! needs a hand-written `mcpServers` block in `qwen-extension.json` that duplicates
//! the extension's registration just to carry three strings.

use ghidra_mcp::config::RawConfig;
use std::path::{Path, PathBuf};

// ── Settings ───────────────────────────────────────────────────────────

/// The settings-file layer, in the shape the shared config resolver consumes.
///
/// Precedence is decided in `ghidra_mcp::cli::layered_config`, not here: CLI
/// flag, then environment, then this. Returning a [`RawConfig`] rather than a
/// bespoke struct is what keeps that single rule single.
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
            // An unknown key is ignored rather than fatal: a settings file
            // written against a newer plugin must not stop the server booting.
            _ => {}
        }
    }
    config
}

/// The settings file this extension reads, relative to a project root.
pub fn settings_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join(".qwen")
        .join("re-ghidra-mcp-qwen.local.md")
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
///
/// Tolerates a leading BOM and CRLF line endings: the file is hand-edited on
/// Windows, where both are routine, and a settings file that silently stops
/// being read because an editor added a BOM is a miserable thing to debug.
fn extract_frontmatter(source: &str) -> Option<&str> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let rest = source.strip_prefix("---")?;
    let rest = rest.strip_prefix('\r').unwrap_or(rest);
    let rest = rest.strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

// ── Preflight ──────────────────────────────────────────────────────────

/// What the environment looks like to the preflight, gathered by the caller.
///
/// The checks themselves are pure so they can be tested without a Ghidra
/// install, a JDK, or a mutated process environment. The binary does the IO;
/// this decides what it means.
#[derive(Debug, Clone, Default)]
pub struct Probe {
    /// Whether `<ghidra_install_dir>/support/analyzeHeadless[.bat]` exists.
    pub launcher_present: bool,
    /// Major version reported by `java -version`, if a `java` was reachable.
    pub java_major: Option<u32>,
}

/// Ghidra 12.1.2 declares `application.java.min=21`; anything older fails at
/// JVM start with a message that does not mention the version.
pub const MIN_JAVA_MAJOR: u32 = 21;

/// Report what is missing, in the order a user should fix it. An empty vector
/// means every cheap check passed — and the hook then says nothing at all.
///
/// Deliberately *cheap*: no JVM spawn, no Ghidra project open, no lock probe.
/// This runs on every session start, so it may cost milliseconds, not seconds.
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

    // The three per-project values. Reported as one finding rather than three:
    // they are always configured together, in the same place.
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
            "Unset: {}. Set them in `.qwen/re-ghidra-mcp-qwen.local.md` \
             (see the extension's examples/ directory) or as \
             GHIDRA_MCP_PROJECT_DIR / GHIDRA_MCP_PROJECT_NAME / \
             GHIDRA_MCP_BOOTSTRAP_PROGRAM.",
            missing.join(", ")
        ));
    }

    findings
}

/// Parse the major version out of `java -version` output.
///
/// The output goes to **stderr**, and the two shapes in the wild are
/// `"21.0.2"` (modern) and `"1.8.0_412"` (legacy, where the major is the
/// second component). Returning `None` for anything unrecognized keeps a
/// surprising vendor string from being read as a version far too low.
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

#[cfg(test)]
mod preflight_tests {
    use super::*;

    fn complete() -> RawConfig {
        RawConfig {
            ghidra_install_dir: Some("C:/ghidra".into()),
            project_dir: Some("C:/re".into()),
            project_name: Some("p".into()),
            bootstrap_program: Some("add.exe".into()),
            ..Default::default()
        }
    }

    fn good_probe() -> Probe {
        Probe {
            launcher_present: true,
            java_major: Some(21),
        }
    }

    /// The silent path. If this ever starts reporting something, the hook goes
    /// from invisible to noisy on every single session.
    #[test]
    fn fully_configured_reports_nothing() {
        assert!(preflight(&complete(), &good_probe()).is_empty());
    }

    #[test]
    fn install_dir_set_but_not_a_ghidra_root_is_reported() {
        let probe = Probe {
            launcher_present: false,
            ..good_probe()
        };
        let f = preflight(&complete(), &probe);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("analyzeHeadless"), "{}", f[0]);
    }

    #[test]
    fn old_java_is_reported_with_its_version() {
        let probe = Probe {
            java_major: Some(17),
            ..good_probe()
        };
        let f = preflight(&complete(), &probe);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("17") && f[0].contains("21"), "{}", f[0]);
    }

    #[test]
    fn absent_java_is_reported() {
        let probe = Probe {
            java_major: None,
            ..good_probe()
        };
        assert!(preflight(&complete(), &probe)[0].contains("No `java` on PATH"));
    }

    /// The three per-project values collapse into one finding, naming exactly
    /// the ones that are missing.
    #[test]
    fn missing_project_values_are_one_finding_naming_each() {
        let cfg = RawConfig {
            project_name: None,
            bootstrap_program: None,
            ..complete()
        };
        let f = preflight(&cfg, &good_probe());
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("project_name"), "{}", f[0]);
        assert!(f[0].contains("bootstrap_program"), "{}", f[0]);
        assert!(!f[0].contains("project_dir,"), "{}", f[0]);
    }

    #[test]
    fn parses_modern_and_legacy_java_version_strings() {
        assert_eq!(
            parse_java_major("openjdk version \"21.0.2\" 2024-01-16"),
            Some(21)
        );
        assert_eq!(
            parse_java_major("java version \"1.8.0_412\"\nJava(TM) SE Runtime"),
            Some(8)
        );
        assert_eq!(
            parse_java_major("openjdk version \"25\" 2025-09-16"),
            Some(25)
        );
    }

    /// Unparseable output must read as "unknown", never as a low version — the
    /// latter would produce a confidently wrong "upgrade your JDK" message.
    #[test]
    fn unrecognized_version_output_is_none() {
        assert_eq!(parse_java_major("command not found"), None);
        assert_eq!(parse_java_major("openjdk version \"weird\""), None);
        assert_eq!(parse_java_major(""), None);
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    const SAMPLE: &str = "---\nproject_dir: C:\\re\\projects\nproject_name: crackme\nbootstrap_program: add.exe\n---\n\n# notes\n";

    #[test]
    fn reads_the_frontmatter_keys() {
        let c = settings_from_markdown(SAMPLE);
        assert_eq!(c.project_dir.as_deref(), Some("C:\\re\\projects"));
        assert_eq!(c.project_name.as_deref(), Some("crackme"));
        assert_eq!(c.bootstrap_program.as_deref(), Some("add.exe"));
        assert_eq!(c.max_heap, None);
    }

    #[test]
    fn crlf_and_bom_still_parse() {
        let crlf = format!("\u{feff}{}", SAMPLE.replace('\n', "\r\n"));
        let c = settings_from_markdown(&crlf);
        assert_eq!(c.project_name.as_deref(), Some("crackme"));
    }

    /// A file with no frontmatter, or an unterminated block, contributes
    /// nothing rather than half-parsing into a confusing partial config.
    #[test]
    fn no_frontmatter_yields_nothing() {
        assert!(settings_from_markdown("# just a heading\n")
            .project_name
            .is_none());
        assert!(settings_from_markdown("---\nproject_name: x\n")
            .project_name
            .is_none());
    }

    /// A key present but blank must not resolve to an empty project name — that
    /// would pass the "is it set?" check and then fail deep inside Ghidra.
    #[test]
    fn blank_value_is_not_a_value() {
        assert!(settings_from_markdown("---\nproject_name:   \n---\n")
            .project_name
            .is_none());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let c = settings_from_markdown("---\nfuture_key: 1\nproject_name: p\n---\n");
        assert_eq!(c.project_name.as_deref(), Some("p"));
    }
}
