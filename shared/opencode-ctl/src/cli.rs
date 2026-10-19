//! Argv routing and the dispatch skeleton.

use crate::host::Host;

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
    match route(argv0, &args) {
        Route::Version => {
            println!("{}", version_line(product, version));
            0
        }
        Route::Usage => {
            eprintln!("{}", usage());
            2
        }
        Route::Sub { .. } | Route::Pair(_) => {
            eprintln!("not implemented ({host:?}, argv0={argv0:?})");
            2
        }
    }
}
