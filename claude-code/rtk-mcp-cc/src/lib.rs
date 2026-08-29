//! Shared logic for the `rtk-mcp-cc` plugin.
//!
//! Everything here is pure and unit-tested: settings resolution, the argument
//! vectors handed to `rtk`, and the hook's pass-through decision. The binaries
//! in `src/bin/` only move bytes and spawn processes.
//!
//! The guiding constraint is that **this plugin owns no rewriting logic**.
//! `rtk` already speaks Claude Code's `PreToolUse` schema natively via
//! `rtk hook claude` — it reads the event on stdin and writes
//! `hookSpecificOutput.updatedInput` on stdout. Re-implementing that here would
//! duplicate knowledge rtk owns and drift the moment rtk changes. So the hook
//! delegates, and this crate's job is only to delegate *safely*.

use std::path::Path;

// ── Settings ───────────────────────────────────────────────────────────

/// Resolved plugin settings.
///
/// Precedence, lowest to highest: built-in defaults, then
/// `.claude/rtk-mcp-cc.local.md` frontmatter, then environment variables.
/// Environment wins so a user can disable or redirect the hook for a single
/// session without editing a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// When false, the hook passes every command through untouched.
    pub enabled: bool,
    /// The `rtk` executable to invoke. Looked up on `PATH` when it has no
    /// path separator.
    pub rtk_bin: String,
    /// Pass `--ultra-compact` to rtk (Level 2 optimizations).
    pub ultra_compact: bool,
    /// Pass `--skip-env` to rtk (sets `SKIP_ENV_VALIDATION=1` for children).
    pub skip_env: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            rtk_bin: "rtk".to_string(),
            ultra_compact: false,
            skip_env: false,
        }
    }
}

impl Config {
    /// Parse settings from the YAML frontmatter of a `.local.md` file.
    ///
    /// Unknown keys and malformed values are ignored: a broken settings file
    /// degrades to the defaults rather than breaking the user's session. This
    /// matters more here than in most plugins, because this hook sits in front
    /// of *every* shell command.
    pub fn from_markdown(source: &str) -> Self {
        let mut config = Config::default();
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
            match key.trim() {
                "enabled" => {
                    if let Some(b) = parse_bool(value) {
                        config.enabled = b;
                    }
                }
                "rtk_bin" => {
                    if !value.is_empty() {
                        config.rtk_bin = value.to_string();
                    }
                }
                "ultra_compact" => {
                    if let Some(b) = parse_bool(value) {
                        config.ultra_compact = b;
                    }
                }
                "skip_env" => {
                    if let Some(b) = parse_bool(value) {
                        config.skip_env = b;
                    }
                }
                _ => {}
            }
        }
        config
    }

    /// Load `<project_dir>/.claude/rtk-mcp-cc.local.md`, then apply the
    /// environment overrides.
    pub fn load(project_dir: &Path) -> Self {
        let path = project_dir.join(".claude").join("rtk-mcp-cc.local.md");
        let mut config = match std::fs::read_to_string(path) {
            Ok(text) => Config::from_markdown(&text),
            Err(_) => Config::default(),
        };
        config.apply_env(|key| std::env::var(key).ok());
        config
    }

    /// Apply environment overrides through an injectable lookup.
    ///
    /// `RTK_BIN` is spelled without a plugin prefix on purpose: it is the same
    /// variable the sibling `rtk-mcp-qwen` extension honours, so a user who
    /// relocates rtk sets one variable for both.
    pub fn apply_env<F>(&mut self, get: F)
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(v) = get("RTK_BIN") {
            if !v.trim().is_empty() {
                self.rtk_bin = v.trim().to_string();
            }
        }
        if let Some(v) = get("RTK_CC_DISABLE") {
            if parse_bool(&v).unwrap_or(false) {
                self.enabled = false;
            }
        }
        if let Some(v) = get("RTK_CC_ULTRA_COMPACT") {
            if let Some(b) = parse_bool(&v) {
                self.ultra_compact = b;
            }
        }
        if let Some(v) = get("RTK_CC_SKIP_ENV") {
            if let Some(b) = parse_bool(&v) {
                self.skip_env = b;
            }
        }
    }

    /// The global rtk flags implied by this config, in a stable order.
    fn global_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if self.ultra_compact {
            flags.push("--ultra-compact".to_string());
        }
        if self.skip_env {
            flags.push("--skip-env".to_string());
        }
        flags
    }
}

/// Accept the spellings people actually write in config files and shells.
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Return the text between the leading `---` fences, if present.
fn extract_frontmatter(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("---")?.trim_start_matches(['\r', '\n']);
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

// ── Hook ───────────────────────────────────────────────────────────────

/// The argument vector for the delegated hook call: `rtk hook claude [flags]`.
pub fn hook_args(config: &Config) -> Vec<String> {
    let mut args = vec!["hook".to_string(), "claude".to_string()];
    args.extend(config.global_flags());
    args
}

/// Decide what this hook writes to stdout.
///
/// `run` receives the rtk argument vector and the raw stdin payload, and
/// returns rtk's stdout on a clean exit — or `None` if rtk could not be
/// spawned, exited non-zero, or produced non-UTF-8.
///
/// Every failure path returns an empty string, which is Claude Code's "no
/// opinion" response: the command runs exactly as the model wrote it. That is
/// the whole safety story of this plugin — a broken or absent rtk must never
/// cost the user a shell command.
///
/// Note that empty output is also rtk's *own* signal for "nothing to rewrite",
/// so the failure path and the no-op path coincide by construction rather than
/// by coincidence.
pub fn delegate<F>(stdin_payload: &str, config: &Config, run: F) -> String
where
    F: FnOnce(&[String], &str) -> Option<String>,
{
    if !config.enabled {
        return String::new();
    }
    if stdin_payload.trim().is_empty() {
        return String::new();
    }

    let Some(stdout) = run(&hook_args(config), stdin_payload) else {
        return String::new();
    };
    if stdout.trim().is_empty() {
        return String::new();
    }

    // Forward only what parses as JSON. rtk writes its advisory notices to
    // stderr, but a future version — or a shim on someone's PATH named `rtk` —
    // could put prose on stdout, and Claude Code treats unparsable hook output
    // as an error. Validating here keeps a stray line from turning into a
    // failed tool call.
    if serde_json::from_str::<serde_json::Value>(stdout.trim()).is_err() {
        tracing::warn!("rtk produced non-JSON stdout; passing the command through unchanged");
        return String::new();
    }
    stdout
}

// ── Analytics argument vectors (MCP tools) ─────────────────────────────

/// Options for the `rtk_gain` tool.
#[derive(Debug, Default, Clone)]
pub struct GainOptions {
    pub history: bool,
    pub project_only: bool,
    pub format: Option<String>,
}

/// Options for the `rtk_discover` tool.
#[derive(Debug, Default, Clone)]
pub struct DiscoverOptions {
    pub project: Option<String>,
    pub limit: Option<u32>,
    pub since: Option<u32>,
    pub all_projects: bool,
    pub format: Option<String>,
}

/// Build the argv for `rtk gain`.
///
/// `--reset` is deliberately unreachable from here. It zeroes the user's saved
/// statistics, and an MCP tool that can silently destroy data is not worth the
/// convenience.
pub fn gain_args(opts: &GainOptions, config: &Config) -> Result<Vec<String>, String> {
    let mut args = vec!["gain".to_string()];
    if opts.history {
        args.push("--history".to_string());
    }
    if opts.project_only {
        args.push("--project".to_string());
    }
    if let Some(fmt) = &opts.format {
        let fmt = validate_format(fmt, &["text", "json", "csv"])?;
        args.push("--format".to_string());
        args.push(fmt);
    }
    args.extend(config.global_flags());
    Ok(args)
}

/// Build the argv for `rtk discover`.
pub fn discover_args(opts: &DiscoverOptions, config: &Config) -> Result<Vec<String>, String> {
    let mut args = vec!["discover".to_string()];
    if let Some(project) = &opts.project {
        if !project.trim().is_empty() {
            args.push("--project".to_string());
            args.push(project.trim().to_string());
        }
    }
    if let Some(limit) = opts.limit {
        args.push("--limit".to_string());
        args.push(limit.to_string());
    }
    if let Some(since) = opts.since {
        args.push("--since".to_string());
        args.push(since.to_string());
    }
    if opts.all_projects {
        args.push("--all".to_string());
    }
    if let Some(fmt) = &opts.format {
        let fmt = validate_format(fmt, &["text", "json"])?;
        args.push("--format".to_string());
        args.push(fmt);
    }
    args.extend(config.global_flags());
    Ok(args)
}

/// Build the argv for `rtk hook check <command>` — a dry run that reports how a
/// command *would* be rewritten, without executing anything.
pub fn check_args(command: &str, config: &Config) -> Result<Vec<String>, String> {
    if command.trim().is_empty() {
        return Err("command must not be empty".to_string());
    }
    let mut args = vec!["hook".to_string(), "check".to_string()];
    args.extend(config.global_flags());
    // Passed as a single argument. `rtk hook check` accepts both a quoted
    // command and a bare argv tail and treats them identically, so the single
    // argument avoids any question of who re-splits the string.
    args.push(command.trim().to_string());
    Ok(args)
}

/// Build the argv for `rtk proxy -- <argv>`.
///
/// This one *executes*. It takes an already-split argument vector rather than a
/// command string precisely so nothing here ever reaches a shell: the words the
/// caller supplies are the words that are executed, and no quoting, globbing,
/// pipeline or `;` is interpreted along the way.
pub fn proxy_args(argv: &[String], config: &Config) -> Result<Vec<String>, String> {
    let cleaned: Vec<String> = argv
        .iter()
        .filter(|a| !a.is_empty())
        .map(|a| a.to_string())
        .collect();
    if cleaned.is_empty() {
        return Err("argv must contain at least the program to run".to_string());
    }
    let mut args = vec!["proxy".to_string()];
    args.extend(config.global_flags());
    args.extend(cleaned);
    Ok(args)
}

fn validate_format(value: &str, allowed: &[&str]) -> Result<String, String> {
    let lowered = value.trim().to_ascii_lowercase();
    if allowed.contains(&lowered.as_str()) {
        Ok(lowered)
    } else {
        Err(format!(
            "unsupported format {value:?}; expected one of {}",
            allowed.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── settings ───────────────────────────────────────────────────

    #[test]
    fn defaults_are_enabled_and_use_path_rtk() {
        let config = Config::default();
        assert!(config.enabled);
        assert_eq!(config.rtk_bin, "rtk");
        assert!(!config.ultra_compact);
        assert!(!config.skip_env);
    }

    #[test]
    fn frontmatter_overrides_defaults() {
        let config = Config::from_markdown(
            "---\nenabled: false\nrtk_bin: /opt/rtk/bin/rtk\nultra_compact: true\n---\n\nprose\n",
        );
        assert!(!config.enabled);
        assert_eq!(config.rtk_bin, "/opt/rtk/bin/rtk");
        assert!(config.ultra_compact);
        assert!(!config.skip_env);
    }

    #[test]
    fn malformed_settings_degrade_to_defaults() {
        // No frontmatter at all, an unknown key, and an unparsable value.
        assert_eq!(
            Config::from_markdown("no frontmatter here"),
            Config::default()
        );
        assert_eq!(
            Config::from_markdown("---\nnonsense: yes\nenabled: banana\n---\n"),
            Config::default()
        );
    }

    #[test]
    fn env_overrides_file() {
        let mut config = Config::from_markdown("---\nenabled: true\nrtk_bin: from-file\n---\n");
        config.apply_env(|key| match key {
            "RTK_BIN" => Some("from-env".to_string()),
            "RTK_CC_DISABLE" => Some("1".to_string()),
            _ => None,
        });
        assert_eq!(config.rtk_bin, "from-env");
        assert!(!config.enabled);
    }

    #[test]
    fn disable_flag_only_disables_when_truthy() {
        let mut config = Config::default();
        config.apply_env(|key| (key == "RTK_CC_DISABLE").then(|| "0".to_string()));
        assert!(config.enabled, "RTK_CC_DISABLE=0 must not disable the hook");
    }

    #[test]
    fn blank_rtk_bin_does_not_erase_the_default() {
        let mut config = Config::default();
        config.apply_env(|key| (key == "RTK_BIN").then(|| "   ".to_string()));
        assert_eq!(config.rtk_bin, "rtk");
    }

    // ── hook argv ──────────────────────────────────────────────────

    #[test]
    fn hook_args_are_bare_by_default() {
        assert_eq!(hook_args(&Config::default()), vec!["hook", "claude"]);
    }

    #[test]
    fn hook_args_carry_flags_in_stable_order() {
        let config = Config {
            ultra_compact: true,
            skip_env: true,
            ..Config::default()
        };
        assert_eq!(
            hook_args(&config),
            vec!["hook", "claude", "--ultra-compact", "--skip-env"]
        );
    }

    // ── hook decision ──────────────────────────────────────────────

    const EVENT: &str = r#"{"tool_name":"Bash","tool_input":{"command":"cat README.md"}}"#;
    const REWRITE: &str = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rtk read README.md"}}}"#;

    #[test]
    fn forwards_rtk_json_verbatim() {
        let out = delegate(EVENT, &Config::default(), |_, _| Some(REWRITE.to_string()));
        assert_eq!(out, REWRITE);
    }

    #[test]
    fn hands_rtk_the_original_payload() {
        let mut seen = String::new();
        delegate(EVENT, &Config::default(), |_, payload| {
            seen = payload.to_string();
            None
        });
        assert_eq!(seen, EVENT, "rtk must receive the event byte-for-byte");
    }

    #[test]
    fn disabled_config_never_spawns_rtk() {
        let config = Config {
            enabled: false,
            ..Config::default()
        };
        let out = delegate(EVENT, &config, |_, _| {
            panic!("rtk must not be spawned when the plugin is disabled")
        });
        assert!(out.is_empty());
    }

    #[test]
    fn empty_stdin_is_a_no_op() {
        let out = delegate("   \n", &Config::default(), |_, _| {
            panic!("rtk must not be spawned for an empty payload")
        });
        assert!(out.is_empty());
    }

    #[test]
    fn missing_rtk_fails_open() {
        // `None` is how the binary reports "could not spawn" / "exited non-zero".
        assert!(delegate(EVENT, &Config::default(), |_, _| None).is_empty());
    }

    #[test]
    fn empty_rtk_output_is_a_no_op() {
        // rtk's own "nothing to rewrite" signal, including the idempotent case
        // where the command was already rewritten by another rtk hook.
        assert!(delegate(EVENT, &Config::default(), |_, _| Some(String::new())).is_empty());
    }

    #[test]
    fn non_json_rtk_output_is_suppressed() {
        let out = delegate(EVENT, &Config::default(), |_, _| {
            Some("rtk: something went sideways\n".to_string())
        });
        assert!(out.is_empty(), "prose on stdout must not reach Claude Code");
    }

    // ── analytics argv ─────────────────────────────────────────────

    #[test]
    fn gain_args_default_to_a_bare_summary() {
        let args = gain_args(&GainOptions::default(), &Config::default()).unwrap();
        assert_eq!(args, vec!["gain"]);
    }

    #[test]
    fn gain_args_accept_history_project_and_format() {
        let opts = GainOptions {
            history: true,
            project_only: true,
            format: Some("JSON".to_string()),
        };
        let args = gain_args(&opts, &Config::default()).unwrap();
        assert_eq!(
            args,
            vec!["gain", "--history", "--project", "--format", "json"]
        );
    }

    #[test]
    fn gain_args_reject_unknown_formats() {
        let opts = GainOptions {
            format: Some("yaml".to_string()),
            ..GainOptions::default()
        };
        assert!(gain_args(&opts, &Config::default()).is_err());
    }

    #[test]
    fn gain_args_can_never_reach_reset() {
        // A guard against someone later adding a passthrough option: whatever
        // the caller asks for, `--reset` must not appear.
        let opts = GainOptions {
            history: true,
            project_only: true,
            format: Some("csv".to_string()),
        };
        let args = gain_args(&opts, &Config::default()).unwrap();
        assert!(!args.iter().any(|a| a == "--reset"));
    }

    #[test]
    fn discover_args_carry_every_option() {
        let opts = DiscoverOptions {
            project: Some("  aiplugins  ".to_string()),
            limit: Some(5),
            since: Some(7),
            all_projects: true,
            format: Some("json".to_string()),
        };
        let args = discover_args(&opts, &Config::default()).unwrap();
        assert_eq!(
            args,
            vec![
                "discover",
                "--project",
                "aiplugins",
                "--limit",
                "5",
                "--since",
                "7",
                "--all",
                "--format",
                "json"
            ]
        );
    }

    #[test]
    fn discover_rejects_csv_which_it_does_not_support() {
        // `rtk gain` takes csv; `rtk discover` does not. The difference is real
        // and this test is what keeps the two from being copy-pasted together.
        let opts = DiscoverOptions {
            format: Some("csv".to_string()),
            ..DiscoverOptions::default()
        };
        assert!(discover_args(&opts, &Config::default()).is_err());
    }

    #[test]
    fn check_args_pass_the_command_as_one_argument() {
        let args = check_args("grep -rn foo src/", &Config::default()).unwrap();
        assert_eq!(args, vec!["hook", "check", "grep -rn foo src/"]);
    }

    #[test]
    fn check_args_reject_an_empty_command() {
        assert!(check_args("   ", &Config::default()).is_err());
    }

    #[test]
    fn proxy_args_preserve_argv_without_splitting() {
        let argv = vec![
            "git".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            "a message; with punctuation".to_string(),
        ];
        let args = proxy_args(&argv, &Config::default()).unwrap();
        assert_eq!(
            args,
            vec![
                "proxy",
                "git",
                "commit",
                "-m",
                "a message; with punctuation"
            ],
            "argv elements must survive intact — no shell is involved"
        );
    }

    #[test]
    fn proxy_args_reject_an_empty_argv() {
        assert!(proxy_args(&[], &Config::default()).is_err());
        assert!(proxy_args(&["".to_string()], &Config::default()).is_err());
    }

    #[test]
    fn global_flags_reach_the_analytics_commands_too() {
        let config = Config {
            ultra_compact: true,
            ..Config::default()
        };
        assert!(gain_args(&GainOptions::default(), &config)
            .unwrap()
            .contains(&"--ultra-compact".to_string()));
        assert!(discover_args(&DiscoverOptions::default(), &config)
            .unwrap()
            .contains(&"--ultra-compact".to_string()));
    }
}
