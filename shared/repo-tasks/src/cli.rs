// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The command line (11): argv routing and the subcommands.

use std::io::{Read, Write};
use std::path::Path;

use crate::archive::{self, Entry as ArchiveEntry, Left};
use crate::clock::SystemClock;
use crate::error::Error;
use crate::host::{self, Host};
use crate::model::Status;
use crate::ops::{self, ListHeader};
use crate::reminder::{self, HookEnv, HostEvent};
use crate::resolve::{locate, start_dir, Entry, Env, Store};
use crate::store::{read_list, Tuning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Version,
    Usage,
    Sub { name: String, rest: Vec<String> },
}

const SUBCOMMANDS: [&str; 4] = ["serve", "reminder", "list", "archive"];

pub fn route(args: &[String]) -> Route {
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
    "usage: serve [--host h] | reminder --event <claude-code|qwen|antigravity|opencode> [--workspace <path>] | list [--archived] [--dir <path>] | archive [--dir <path>] | --version"
        .to_string()
}

/// What a run reads and writes, so tests can drive it in-process.
pub struct Io<'a> {
    pub env: Env,
    pub hook_env: HookEnv,
    pub stdin: &'a mut dyn Read,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// The process entry point: real environment, standard input and output.
pub async fn dispatch(
    host: Host,
    product: &str,
    version: &str,
    _argv0: Option<&str>,
    args: Vec<String>,
) -> i32 {
    let mut stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    let mut io = Io {
        env: Env::from_process(),
        hook_env: HookEnv::from_process(),
        stdin: &mut stdin,
        out: &mut out,
        err: &mut err,
    };
    run(host, product, version, args, &mut io).await
}

/// The parsed flags of one subcommand.
#[derive(Debug, Default)]
struct Flags {
    values: Vec<(String, String)>,
    switches: Vec<String>,
}

impl Flags {
    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn has(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }
}

/// `--name value` or `--name=value` for `valued`, bare `--name` for `switches`; anything else
/// (including `--dir` on `reminder`) is an error.
fn parse_flags(rest: &[String], valued: &[&str], switches: &[&str]) -> Result<Flags, String> {
    let mut flags = Flags::default();
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        if switches.contains(&name) && inline.is_none() {
            flags.switches.push(name.to_string());
        } else if valued.contains(&name) {
            let v = match inline {
                Some(v) => v,
                None => it
                    .next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value"))?,
            };
            if flags.value(name).is_some() {
                return Err(format!("{name} given twice"));
            }
            flags.values.push((name.to_string(), v));
        } else {
            return Err(format!("unknown argument '{arg}'"));
        }
    }
    Ok(flags)
}

fn say(w: &mut dyn Write, text: &str) {
    let _ = writeln!(w, "{text}");
    let _ = w.flush();
}

fn one_line(e: &Error) -> String {
    e.to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Routes `args` and runs the subcommand; the exit code.
pub async fn run(
    host: Host,
    product: &str,
    version: &str,
    args: Vec<String>,
    io: &mut Io<'_>,
) -> i32 {
    match route(&args) {
        Route::Version => {
            say(io.out, &version_line(product, version));
            0
        }
        Route::Usage => {
            say(io.err, &usage());
            2
        }
        Route::Sub { name, rest } => {
            let done = match name.as_str() {
                "serve" => serve(host, version, &rest, io).await,
                "reminder" => reminder_cmd(&rest, io),
                "list" => list_cmd(&rest, io),
                _ => archive_cmd(&rest, io),
            };
            match done {
                Ok(()) => 0,
                Err((code, msg)) => {
                    say(io.err, &msg);
                    code
                }
            }
        }
    }
}

type Failure = (i32, String);

fn usage_error(msg: String) -> Failure {
    (2, format!("repo-tasks: {msg}\n{}", usage()))
}

fn engine_error(e: &Error) -> Failure {
    (1, format!("repo-tasks: {}", one_line(e)))
}

async fn serve(host: Host, version: &str, rest: &[String], io: &mut Io<'_>) -> Result<(), Failure> {
    let flags = parse_flags(rest, &["--host"], &[]).map_err(usage_error)?;
    let host = host::resolve(host, flags.value("--host"), None).map_err(|e| engine_error(&e))?;
    crate::server::serve(host, version, io.env.clone())
        .await
        .map_err(|e| engine_error(&e))
}

fn reminder_cmd(rest: &[String], io: &mut Io<'_>) -> Result<(), Failure> {
    let flags = parse_flags(rest, &["--event", "--workspace"], &[]).map_err(usage_error)?;
    let event = match flags.value("--event") {
        None => return Err(usage_error("reminder needs --event <host>".to_string())),
        Some(v) => HostEvent::parse(v)
            .ok_or_else(|| usage_error(format!("unknown --event value '{v}'")))?,
    };
    // Draining standard input is not optional: the host writes the event there.
    let mut raw = String::new();
    if event.reads_stdin() {
        let _ = io.stdin.read_to_string(&mut raw);
    }
    match reminder::output(
        event,
        &io.env,
        &io.hook_env,
        &raw,
        flags.value("--workspace"),
    ) {
        Some(text) => say(io.out, &text),
        // Antigravity's hooks "must output their result as a JSON object on stdout"
        // (hooks.md:145-147); the other hosts take silence.
        None if event == HostEvent::Antigravity => say(io.out, "{}"),
        None => {}
    }
    Ok(())
}

/// D for a person's subcommand (7.1), then the store.
fn store_for(flags: &Flags, io: &Io<'_>, host: Host) -> Result<Store, Failure> {
    let dir = flags.value("--dir").map(Path::new);
    let d = start_dir(Entry::Subcommand, &io.env, dir, None, None, host)
        .map_err(|e| engine_error(&e))?;
    locate(&d.path, &io.env).map_err(|e| engine_error(&e))
}

fn list_cmd(rest: &[String], io: &mut Io<'_>) -> Result<(), Failure> {
    let flags = parse_flags(rest, &["--dir"], &["--archived"]).map_err(usage_error)?;
    let store = store_for(&flags, io, Host::Claude)?;
    let tuning = Tuning::default();
    let header = ListHeader::for_store(&store);
    if !flags.has("--archived") {
        let text = ops::list(&store, &tuning, &header).map_err(|e| engine_error(&e))?;
        say(io.out, &text);
        return Ok(());
    }
    // The list first, the archive second: an id still in the list is not shown.
    let list = read_list(&store, &tuning).map_err(|e| engine_error(&e))?;
    let entries = archive::read_archive(&store, &list).map_err(|e| engine_error(&e))?;
    let mut lines = vec![header.0];
    if entries.is_empty() {
        lines.push("no archived tasks".to_string());
    }
    lines.extend(entries.iter().map(archived_line));
    say(io.out, &lines.join("\n"));
    Ok(())
}

/// The form of 9.5 with the mark the task had when it left, without the blocked-by and
/// in-progress parts, ending in how and when it left.
fn archived_line(e: &ArchiveEntry) -> String {
    let t = &e.task;
    let mark = match t.status {
        Status::Pending => "[ ]",
        Status::InProgress => "[>]",
        Status::Completed => "[x]",
    };
    let mut line = format!("#{} {mark} {}", t.id, t.subject);
    if let Some(o) = &t.owner {
        line.push_str(&format!(" [owner {o}]"));
    }
    match &e.how {
        Left::Archived(at) => line.push_str(&format!(" [archived {at}]")),
        Left::Deleted(at) => line.push_str(&format!(" [deleted {at}]")),
    }
    line
}

fn archive_cmd(rest: &[String], io: &mut Io<'_>) -> Result<(), Failure> {
    let flags = parse_flags(rest, &["--dir"], &[]).map_err(usage_error)?;
    let store = store_for(&flags, io, Host::Claude)?;
    let n = archive::archive_completed(&store, &Tuning::default(), &SystemClock)
        .map_err(|e| engine_error(&e))?;
    let text = match n {
        0 => "no completed tasks".to_string(),
        1 => "archived 1 task".to_string(),
        n => format!("archived {n} tasks"),
    };
    say(io.out, &text);
    Ok(())
}
