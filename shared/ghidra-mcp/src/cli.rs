//! The argv-level entry point every agent's plugin binary dispatches into.
//!
//! The plugin binaries (`re-ghidra-cc-mcp` today; the agy and qwen ports later)
//! own nothing but a `main` that hands its argv here. Flag parsing, config
//! precedence, log initialization and the serve loop all live in this crate, so
//! three front ends cannot drift into three subtly different config contracts.
//!
//! `product` is the invoked binary's own name. It appears only in the usage
//! banner and error prefixes — never in an env var or a path — so each agent's
//! diagnostics name the binary the user actually ran.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Dispatch `args` (everything after argv[0]) and return the process exit code.
///
/// `file_config` is the agent's own settings-file layer, sitting below the
/// environment in precedence. Each agent stores settings differently — Claude
/// Code uses `.claude/<plugin>.local.md`, Qwen and agy will use their own — so
/// the *format* is the plugin's business while the *precedence* is decided once,
/// here. Pass `RawConfig::default()` for an agent with no settings file.
///
/// Exit codes are part of the contract with the launching agent: 0 success,
/// 1 serve terminated with an error, 2 bad configuration or usage, 3 the log
/// file could not be opened.
pub async fn dispatch(
    product: &str,
    version: &str,
    args: Vec<String>,
    file_config: crate::config::RawConfig,
) -> i32 {
    let mut it = args.into_iter();
    match it.next().as_deref() {
        Some("serve") => match run_serve(product, it.collect(), file_config).await {
            Ok(()) => 0,
            Err(code) => code,
        },
        Some("emit-skill") => {
            // stdout is safe here: `emit-skill` is a one-shot maintainer utility,
            // NOT the `serve` MCP JSON-RPC channel. `print!` (no trailing newline)
            // keeps the output byte-identical to the committed plugin copy, so
            // `<bin> emit-skill > SKILL.md` round-trips exactly. Pinned by the
            // plugin's tests/skill_emit.rs.
            print!("{}", crate::skill_asset::emit_bytes());
            0
        }
        Some("boot-smoke") => {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .init();
            boot_smoke().await;
            0
        }
        _ => {
            usage(product, version);
            0
        }
    }
}

/// Version + usage, on stderr. Never stdout: an agent that launched this binary
/// as an MCP server is reading stdout as JSON-RPC, and a usage banner there
/// looks like a protocol violation rather than a mistyped command.
fn usage(product: &str, version: &str) {
    eprintln!("{product} {version}");
    eprintln!(
        "usage: {product} serve        (env: GHIDRA_INSTALL_DIR, GHIDRA_MCP_PROJECT_DIR, \
         GHIDRA_MCP_PROJECT_NAME, GHIDRA_MCP_BOOTSTRAP_PROGRAM)"
    );
    eprintln!("       {product} emit-skill  (writes the embedded driver skill to stdout)");
    eprintln!("       {product} boot-smoke  (dev; env: GHIDRA_INSTALL_DIR, GHIDRA_MCP_FIXTURE_*)");
}

/// Resolve config (CLI > env > default), initialize file logging, and serve.
/// `Err(code)` carries the exit code; the reason is reported on STDERR, never
/// stdout — stdout is the MCP channel.
async fn run_serve(
    product: &str,
    cli: Vec<String>,
    file_config: crate::config::RawConfig,
) -> Result<(), i32> {
    let cfg = match resolve_config(cli, file_config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{product} serve: configuration error: {e}");
            return Err(2);
        }
    };
    let log_path = crate::paths::instance_log_path();
    let _guard = match crate::logging::init_file_logging(&log_path) {
        Ok(g) => g,
        Err(e) => {
            eprintln!(
                "{product} serve: cannot initialize log at {}: {e}",
                log_path.display()
            );
            return Err(3);
        }
    };
    tracing::info!(?log_path, product, "serve starting");
    if let Err(e) = crate::server::serve(cfg).await {
        tracing::error!("serve terminated: {e}");
        return Err(1);
    }
    Ok(())
}

/// Layer CLI flags over the environment over the agent's settings file, then
/// validate the result.
///
/// A CLI flag that was silently ignored would be a config trap, so flags DO
/// layer over the environment rather than being an either/or — and the settings
/// file sits below both, so a user can override a committed project setting for
/// one session without editing it.
pub fn resolve_config(
    cli: Vec<String>,
    file_config: crate::config::RawConfig,
) -> Result<crate::config::ServerConfig, crate::config::ConfigError> {
    layer(parse_flags(&cli), file_config, env_or_none).resolve()
}

/// The precedence rule itself, with the environment injected so it is testable
/// without mutating the process environment (which races under a parallel test
/// runner).
fn layer<F>(
    flags: HashMap<String, String>,
    file: crate::config::RawConfig,
    get_env: F,
) -> crate::config::RawConfig
where
    F: Fn(&str) -> Option<String>,
{
    // The empty-string filter lives HERE rather than in the caller's getter, so
    // the "an empty variable does not shadow the file" property holds for every
    // caller instead of depending on each one remembering to filter.
    let pick = |flag: &str, env: &str, from_file: Option<String>| {
        flags
            .get(flag)
            .cloned()
            .or_else(|| get_env(env).filter(|s| !s.is_empty()))
            .or(from_file)
    };
    crate::config::RawConfig {
        ghidra_install_dir: pick(
            "ghidra-install-dir",
            "GHIDRA_INSTALL_DIR",
            file.ghidra_install_dir,
        ),
        project_dir: pick("project-dir", "GHIDRA_MCP_PROJECT_DIR", file.project_dir),
        project_name: pick("project-name", "GHIDRA_MCP_PROJECT_NAME", file.project_name),
        bootstrap_program: pick(
            "bootstrap-program",
            "GHIDRA_MCP_BOOTSTRAP_PROGRAM",
            file.bootstrap_program,
        ),
        bootstrap_program_path: pick(
            "bootstrap-program-path",
            "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH",
            file.bootstrap_program_path,
        ),
        max_heap: pick("max-heap", "GHIDRA_MCP_MAX_HEAP", file.max_heap),
    }
}

fn env_or_none(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Parse `--key value` and `--key=value` into a map. Non-flag tokens are
/// skipped; unrecognized flags are collected rather than rejected, so they
/// simply match no `pick(..)` above (forward compatibility).
fn parse_flags(args: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(rest) = args[i].strip_prefix("--") {
            if let Some((k, v)) = rest.split_once('=') {
                map.insert(k.to_string(), v.to_string());
                i += 1;
            } else if i + 1 < args.len() {
                map.insert(rest.to_string(), args[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    map
}

/// Dev-only: boot the worker and drive attach + decompile end to end, logging
/// to stderr. Needs a real Ghidra install and the `GHIDRA_MCP_FIXTURE_*` vars.
async fn boot_smoke() {
    let script_dir = std::env::temp_dir().join("ghidra-mcp-smoke-scripts");
    crate::worker_asset_extract(&script_dir);
    let cfg = ghidra_worker_ctl::config::WorkerConfig {
        ghidra_install_dir: PathBuf::from(env("GHIDRA_INSTALL_DIR")),
        project_dir: PathBuf::from(env("GHIDRA_MCP_FIXTURE_PROJECT_DIR")),
        project_name: env("GHIDRA_MCP_FIXTURE_PROJECT_NAME"),
        bootstrap_program: env("GHIDRA_MCP_FIXTURE_PROGRAM"),
        script_dir,
        script_name: crate::worker_asset::WORKER_SCRIPT_NAME.to_string(),
        max_heap: None,
        boot_timeout: Duration::from_secs(120),
    };
    let mut w = ghidra_worker_ctl::boot::boot_worker(&cfg)
        .await
        .expect("worker boots (if it hangs on Windows, check EDR/firewall on the loopback bind)");
    eprintln!("booted worker: {:?}", w.announce);
    let program = env("GHIDRA_MCP_FIXTURE_PROGRAM");
    let func = env("GHIDRA_MCP_FIXTURE_FUNCTION");
    let attach = w
        .conn
        .request(&mk(
            1,
            "attach_program",
            serde_json::json!({ "program_path": program }),
        ))
        .await
        .unwrap();
    eprintln!("attach: {attach:?}");
    let dec = w
        .conn
        .request(&mk(
            2,
            "decompile_function",
            serde_json::json!({ "name": func }),
        ))
        .await
        .unwrap();
    eprintln!("decompile: {dec:?}");
    let _ = w.conn.send(&mk(3, "shutdown", serde_json::json!({}))).await;
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing env var {name}"))
}

fn mk(id: i64, method: &str, params: serde_json::Value) -> ghidra_ipc::rpc::RpcRequest {
    ghidra_ipc::rpc::RpcRequest {
        jsonrpc: ghidra_ipc::rpc::JsonRpcVersion,
        id: ghidra_ipc::rpc::RpcId::Number(id),
        method: method.to_string(),
        params: Some(params),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_flag_spellings() {
        let f = parse_flags(&[
            "--project-name".into(),
            "proj".into(),
            "--max-heap=4G".into(),
        ]);
        assert_eq!(f.get("project-name").map(String::as_str), Some("proj"));
        assert_eq!(f.get("max-heap").map(String::as_str), Some("4G"));
    }

    /// A trailing `--flag` with no value must not consume past the end or panic.
    #[test]
    fn dangling_flag_is_dropped_not_panicked() {
        let f = parse_flags(&["--project-name".into()]);
        assert!(f.is_empty());
    }

    /// Non-flag tokens are skipped rather than mistaken for a value.
    #[test]
    fn positional_tokens_are_ignored() {
        let f = parse_flags(&["stray".into(), "--max-heap=2G".into()]);
        assert_eq!(f.len(), 1);
        assert_eq!(f.get("max-heap").map(String::as_str), Some("2G"));
    }

    fn file_layer() -> crate::config::RawConfig {
        crate::config::RawConfig {
            project_name: Some("from-file".into()),
            project_dir: Some("dir-from-file".into()),
            ..Default::default()
        }
    }

    /// The whole point of the three layers: a flag beats the environment, which
    /// beats the settings file. Getting this backwards would make a committed
    /// project setting un-overridable for a single session.
    #[test]
    fn flag_beats_env_beats_file() {
        let flags = parse_flags(&["--project-name".into(), "from-flag".into()]);
        let env = |k: &str| match k {
            "GHIDRA_MCP_PROJECT_NAME" => Some("from-env".to_string()),
            "GHIDRA_MCP_PROJECT_DIR" => Some("dir-from-env".to_string()),
            _ => None,
        };
        let merged = layer(flags, file_layer(), env);
        assert_eq!(merged.project_name.as_deref(), Some("from-flag"));
        assert_eq!(merged.project_dir.as_deref(), Some("dir-from-env"));
    }

    /// With no flag and no environment, the settings file is what supplies the
    /// value — this is the path that lets a project be configured without a
    /// hand-written per-repo MCP registration.
    #[test]
    fn file_layer_supplies_values_nothing_else_set() {
        let merged = layer(HashMap::new(), file_layer(), |_| None);
        assert_eq!(merged.project_name.as_deref(), Some("from-file"));
        assert_eq!(merged.project_dir.as_deref(), Some("dir-from-file"));
        assert_eq!(merged.bootstrap_program, None);
    }

    /// An empty environment variable must not shadow the settings file. Windows
    /// launchers routinely pass `VAR=` for an unset value, and treating that as
    /// "set to empty" would silently blank a working configuration.
    #[test]
    fn empty_env_does_not_shadow_the_file() {
        let merged = layer(HashMap::new(), file_layer(), |k| {
            (k == "GHIDRA_MCP_PROJECT_NAME").then(String::new)
        });
        assert_eq!(merged.project_name.as_deref(), Some("from-file"));
    }
}
