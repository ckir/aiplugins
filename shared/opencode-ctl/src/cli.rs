//! Argv routing and the dispatch skeleton.

use std::sync::Arc;

use rmcp::ServiceExt;

use crate::clock::{Clock, SystemClock};
use crate::engine::{Engine, EnvSnapshot};
use crate::host::{self, Host};
use crate::liveness::{ProcessProbe, SystemProbe};
use crate::logging::LogScope;
use crate::pairing;
use crate::server::OpencodeCtlServer;
use crate::settings;
use crate::startup::startup;
use crate::transport::{build_transport, home_required};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Version,
    Usage,
    Sub { name: String, rest: Vec<String> },
    Pair(Vec<String>),
}

const SUBCOMMANDS: [&str; 8] = [
    "serve",
    "pair",
    "resume",
    "attach",
    "gc",
    "install-launcher",
    "emit-skill",
    "doctor-json",
];

/// The ONE empty-is-unset rule: a value empty after trimming is unset; any other value is returned verbatim.
pub fn blank_is_unset(v: Option<String>) -> Option<String> {
    v.filter(|v| !v.trim().is_empty())
}

pub fn env_nonempty(name: &str) -> Option<String> {
    blank_is_unset(std::env::var(name).ok())
}

pub fn route(_argv0: Option<&str>, args: &[String]) -> Route {
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => Route::Version,
        Some(w) if SUBCOMMANDS.contains(&w) => Route::Sub {
            name: w.to_string(),
            rest: args[1..].to_vec(),
        },
        _ => Route::Usage,
    }
}

pub fn version_line(product: &str, version: &str) -> String {
    format!("{product} {version}")
}

pub fn usage() -> String {
    "usage: serve [--host h] | pair | resume | attach | gc | install-launcher | emit-skill <name> --host <h> | doctor-json [verbose] [--host h] | --version"
        .to_string()
}

pub async fn dispatch(
    host: Host,
    product: &str,
    version: &str,
    argv0: Option<&str>,
    args: Vec<String>,
) -> i32 {
    let r = route(argv0, &args);
    let user_home = match home_required(&r, &|k| env_nonempty(k)) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", e.message);
            return 1;
        }
    };
    match r {
        Route::Version => {
            println!("{}", version_line(product, version));
            0
        }
        Route::Usage => {
            eprintln!("{}", usage());
            2
        }
        Route::Sub { name, rest } if name == "serve" => {
            match serve(host, &user_home, &rest).await {
                Ok(()) => 0,
                Err(msg) => {
                    eprintln!("{msg}");
                    1
                }
            }
        }
        Route::Sub { .. } | Route::Pair(_) => {
            eprintln!("not implemented ({host:?}, argv0={argv0:?})");
            2
        }
    }
}

/// What `serve` resolves before it starts logging: settings loaded with the RAW environment (an
/// empty `OPENCODE_CTL_SERVER` must stay visible as the background-service override), the
/// pairing identity resolved and applied (`pairing.rs`), and the warnings for the log.
pub struct ServePrep {
    pub settings: settings::Settings,
    pub notes: Vec<settings::Note>,
    pub env: EnvSnapshot,
    pub identity_warnings: Vec<crate::error::ToolError>,
}

pub fn prepare_serve(
    host: Host,
    ctl_home: &std::path::Path,
    cwd: std::path::PathBuf,
    get: &dyn Fn(&str) -> Option<String>,
) -> ServePrep {
    let mut env = EnvSnapshot::from_lookup(get, cwd, ctl_home.to_path_buf());
    let (mut settings, notes) = settings::load_from_fs(host, ctl_home, &env.cwd, get);
    let (identity, identity_warnings) = pairing::resolve_identity(&env, ctl_home);
    pairing::apply_identity(&mut env, &mut settings, &identity);
    ServePrep {
        settings,
        notes,
        env,
        identity_warnings,
    }
}

async fn serve(host: Host, user_home: &std::path::Path, rest: &[String]) -> Result<(), String> {
    let host_arg = match rest {
        [] => None,
        [flag, value] if flag == "--host" => Some(value.as_str()),
        _ => return Err(usage()),
    };
    let env_host = env_nonempty("OPENCODE_CTL_HOST");
    let host = host::resolve(host, host_arg, env_host.as_deref()).map_err(|e| e.message)?;
    let ctl_home = settings::ctl_home(&|k| env_nonempty(k));
    let cwd = std::env::current_dir().unwrap_or_default();
    let prep = prepare_serve(host, &ctl_home, cwd, &|k| std::env::var(k).ok());
    let probe: Arc<dyn ProcessProbe> = Arc::new(SystemProbe);
    let debug = env_nonempty("OPENCODE_CTL_LOG").is_some_and(|v| v.eq_ignore_ascii_case("debug"));
    let report = startup(
        &ctl_home,
        host,
        &prep.env,
        probe.as_ref(),
        SystemClock.now_ms(),
        debug,
        LogScope::Global,
        &prep.identity_warnings,
    );
    if let Some(w) = &report.log_warning {
        eprintln!("{w}");
    }
    for line in &report.identity_stderr {
        eprintln!("opencode-ctl: {line}");
    }
    let transport = build_transport(&prep.settings, &ctl_home, user_home);
    let engine = Arc::new(Engine::new(
        prep.settings,
        prep.notes,
        host,
        prep.env,
        transport,
        SystemClock,
        probe,
    ));
    let running = OpencodeCtlServer::new(engine)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| e.to_string())?;
    running.waiting().await.map_err(|e| e.to_string())?;
    drop(report);
    Ok(())
}
