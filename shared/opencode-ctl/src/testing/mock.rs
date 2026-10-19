//! Mock `opencode` binary logic (Task 12). `tests/bin/mock_opencode.rs` is a shim over [`main`].
//!
//! Test infrastructure only: it reads keyed fixtures from `MOCK_OPENCODE_FIXTURES`, keeps
//! counters and an argv log in `MOCK_OPENCODE_STATE_DIR`, and never touches the real home, the
//! network or a real `opencode`.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::openapi::{OperationTable, TableSource};

/// The captured real stderr of `opencode api server.info --server http://127.0.0.1:1`
/// (opencode v2.0.32; the raised error is `server-connection.ts:96`).
pub const UNREACHABLE_STDERR: &str = r#"Error: Could not reach server at http://127.0.0.1:1
    at cli.server-connection.resolve (/$bunfs/root/chunk-sjncn5eg.js:2:1149)
    at cli.server-connection.resolve (definition) (/$bunfs/root/chunk-bsm4erxq.js:2:402)
    at cli.api (/$bunfs/root/chunk-7wjtqsrz.js:7:21275)
    at cli.api (definition) (/$bunfs/root/chunk-sjncn5eg.js:2:1109) {
  [cause]: ClientError: Transport: Unable to connect. Is the computer able to access the url?
      at <anonymous> (/$bunfs/root/chunk-71dqgg9j.js:2:1017)
      at async <anonymous> (/$bunfs/root/chunk-71dqgg9j.js:2:1265)
      at processTicksAndRejections (native:7:39) {
    [cause]: TypeError: Unable to connect. Is the computer able to access the url?
        at async <anonymous> (/$bunfs/root/chunk-71dqgg9j.js:2:981)
        at async <anonymous> (/$bunfs/root/chunk-71dqgg9j.js:2:1265)
  }
}
"#;

const DEFAULT_URL: &str = "http://127.0.0.1:49374";
const PIPE_CAP: usize = 262_144;
const DEFAULT_BIG: usize = 300_000;

/// Percent-encode every byte outside `[A-Za-z0-9_.-]` as uppercase `%XX`.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `<op>__<k>=<v>&...`: pairs sorted by name, values (not names) percent-encoded.
/// With no params the key is the bare operation id.
pub fn fixture_key(op: &str, params: &[(String, String)]) -> String {
    if params.is_empty() {
        return op.to_string();
    }
    let mut sorted: Vec<&(String, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let pairs: Vec<String> = sorted
        .iter()
        .map(|(k, v)| format!("{k}={}", percent_encode(v)))
        .collect();
    format!("{op}__{}", pairs.join("&"))
}

/// Pick the fixture file under `<fixtures>/mock/`, most specific first:
/// `<op>__<params>__<n>`, `<op>__<params>`, `<op>__<n>`, `<op>`. A numbered family
/// past its last file repeats that last file.
pub fn select_fixture(
    fixtures: &Path,
    op: &str,
    params: &[(String, String)],
    n: u32,
) -> Option<PathBuf> {
    let dir = fixtures.join("mock");
    let numbered = |stem: &str| {
        (1..=n.max(1))
            .rev()
            .map(|m| dir.join(format!("{stem}__{m}.json")))
            .find(|p| p.is_file())
    };
    let plain = |stem: &str| Some(dir.join(format!("{stem}.json"))).filter(|p| p.is_file());
    let with_params = fixture_key(op, params);
    if with_params != op {
        if let Some(p) = numbered(&with_params).or_else(|| plain(&with_params)) {
            return Some(p);
        }
    }
    numbered(op).or_else(|| plain(op))
}

/// `MOCK_OPENCODE_MODE` = `<key>:<mode>[,...]`.
pub fn parse_mode(spec: &str) -> Vec<(String, String)> {
    spec.split(',')
        .filter_map(|part| {
            let (k, m) = part.split_once(':')?;
            (!k.is_empty() && !m.is_empty()).then(|| (k.trim().to_string(), m.trim().to_string()))
        })
        .collect()
}

/// Whether fd 1 is a pipe (the mock truncates `big` output only then).
#[cfg(unix)]
pub fn is_stdout_pipe() -> bool {
    use std::os::fd::AsFd;
    use std::os::unix::fs::FileTypeExt;
    std::io::stdout()
        .as_fd()
        .try_clone_to_owned()
        .ok()
        .and_then(|fd| fs::File::from(fd).metadata().ok())
        .is_some_and(|m| m.file_type().is_fifo())
}

#[cfg(windows)]
pub fn is_stdout_pipe() -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE};
    let handle = HANDLE(std::io::stdout().as_raw_handle());
    // SAFETY: the handle is the process's own stdout handle, valid for the call.
    unsafe { GetFileType(handle) == FILE_TYPE_PIPE }
}

#[cfg(not(any(unix, windows)))]
pub fn is_stdout_pipe() -> bool {
    false
}

/// One argv-log line (no trailing newline): `{argv, env:{<name>:{present,len,sha8}}, stdout_kind}`
/// for `OPENCODE_PASSWORD`, `OPENCODE_SERVER_PASSWORD` and every `OPENCODE_CTL_*`; never a value.
pub fn env_presence_line(argv: &[String], env: &[(String, String)], stdout_kind: &str) -> String {
    let mut seen: BTreeMap<String, Value> = BTreeMap::new();
    for fixed in ["OPENCODE_PASSWORD", "OPENCODE_SERVER_PASSWORD"] {
        seen.insert(
            fixed.to_string(),
            json!({"present": false, "len": 0, "sha8": null}),
        );
    }
    for (k, v) in env {
        if k == "OPENCODE_PASSWORD"
            || k == "OPENCODE_SERVER_PASSWORD"
            || k.starts_with("OPENCODE_CTL_")
        {
            let digest = Sha256::digest(v.as_bytes());
            let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
            seen.insert(
                k.clone(),
                json!({"present": true, "len": v.len(), "sha8": sha8}),
            );
        }
    }
    let env_obj: Map<String, Value> = seen.into_iter().collect();
    json!({"argv": argv, "env": env_obj, "stdout_kind": stdout_kind}).to_string()
}

/// The first `required` property of the operation's JSON body that `body` lacks.
pub fn validate_body(table: &OperationTable, op: &str, body: &Value) -> Option<String> {
    let operation = table.get(op)?;
    operation
        .body_required
        .iter()
        .find(|prop| body.get(prop.as_str()).is_none())
        .cloned()
}

struct Reply {
    exit: i32,
    stdout: String,
    stderr: String,
}

impl Reply {
    fn new(exit: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            exit,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    fn http(status_line: &str, tag: &str, message: &str) -> Self {
        Self::new(1, &tagged(tag, message), status_line)
    }
}

fn tagged(tag: &str, message: &str) -> String {
    json!({"_tag": tag, "message": message}).to_string()
}

fn unknown_error_stack(cause: &str) -> String {
    format!(
        "UnknownError\n    at cli.api (/$bunfs/root/chunk-7wjtqsrz.js:7:21275)\n    at cli.api (definition) (/$bunfs/root/chunk-sjncn5eg.js:2:1109) {{\n  [cause]: {cause}\n      at <anonymous> (/$bunfs/root/chunk-7wjtqsrz.js:7:21275)\n}}\n"
    )
}

fn now_ms(env: &dyn Fn(&str) -> Option<String>) -> i64 {
    env("MOCK_OPENCODE_NOW_MS")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        })
}

/// Expand whole-string `"{{now}}"`, `"{{now-N}}"`, `"{{now+N}}"` to numbers.
fn expand_now(v: &mut Value, now: i64) {
    match v {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix("{{now").and_then(|r| r.strip_suffix("}}")) {
                let delta = match rest {
                    "" => Some(0),
                    r if r.starts_with('-') || r.starts_with('+') => r[1..]
                        .parse::<i64>()
                        .ok()
                        .map(|n| if r.starts_with('-') { -n } else { n }),
                    _ => None,
                };
                if let Some(d) = delta {
                    *v = json!(now + d);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| expand_now(x, now)),
        Value::Object(o) => o.values_mut().for_each(|x| expand_now(x, now)),
        _ => {}
    }
}

/// Claim the next 1-based sequence number for `key`: the first free `<key>.<n>.seen` marker.
fn next_seq(state: &Path, key: &str) -> u32 {
    let mut n = 1;
    loop {
        let marker = state.join(format!("{key}.{n}.seen"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            Ok(_) => return n,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(_) => return n,
        }
    }
}

/// Count one poll of `key` (append one byte; the file length is the count).
fn count_poll(state: &Path, key: &str) -> u64 {
    let path = state.join(format!("{key}.polls"));
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open(&path) {
        let _ = f.write_all(b".");
    }
    fs::metadata(&path).map(|m| m.len()).unwrap_or(1)
}

fn reply_from_fixture(path: &Path, now: i64) -> Reply {
    let text = fs::read_to_string(path).unwrap_or_default();
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        // Not JSON (e.g. the bare service URL): stdout verbatim, exit 0.
        return Reply::new(0, &text, "");
    };
    expand_now(&mut value, now);
    let stream = |v: Option<&Value>| match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    };
    match value.get("exit").and_then(Value::as_i64) {
        Some(exit) => Reply {
            exit: exit as i32,
            stdout: stream(value.get("stdout")),
            stderr: stream(value.get("stderr")),
        },
        None => Reply::new(0, &value.to_string(), ""),
    }
}

/// A non-empty permission or form list row for the polling modes.
fn pending_row(kind: &str, params: &[(String, String)]) -> Value {
    let session = params
        .iter()
        .find(|(k, _)| k == "sessionID")
        .map_or("ses_aaaaaaaaaaaaaaaaaaaaaaaaaa", |(_, v)| v.as_str());
    let row = if kind == "form" {
        json!({"id": "frm_mock1", "sessionID": session, "title": "mock form",
               "fields": [{"key": "answer", "type": "string"}]})
    } else {
        json!({"id": "per_mock1", "sessionID": session, "action": "edit",
               "resources": ["/tmp/w/file.txt"]})
    };
    json!({"data": [row], "cursor": {"previous": null, "next": null}})
}

fn big_array(n: usize) -> Vec<u8> {
    // `[` + padding + `0,` * k + `0` + `]` is exactly n bytes and valid JSON for n >= 3.
    let n = n.max(3);
    let pad = (n - 3) % 2;
    let k = (n - 3 - pad) / 2;
    let mut v = Vec::with_capacity(n);
    v.push(b'[');
    v.extend(std::iter::repeat_n(b' ', pad));
    for _ in 0..k {
        v.extend_from_slice(b"0,");
    }
    v.extend_from_slice(b"0]");
    v
}

fn write_stdout(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}

fn spawn_grandchild(state: &Path) {
    #[cfg(unix)]
    let mut cmd = {
        let mut c = Command::new("sleep");
        c.arg("300");
        c
    };
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("ping");
        c.args(["-n", "300", "127.0.0.1"]);
        c
    };
    #[cfg(not(any(unix, windows)))]
    let mut cmd = Command::new("sleep");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(child) = cmd.spawn() {
        let _ = fs::write(state.join("grandchild.pid"), child.id().to_string());
    }
}

struct Ctx<'a> {
    fixtures: &'a Path,
    state: &'a Path,
    modes: Vec<(String, String)>,
    now: i64,
    after_polls: u64,
    big_bytes: usize,
}

enum Outcome {
    Reply(Reply),
    Big,
    Slow,
}

impl Ctx<'_> {
    fn mode_of(&self, key: &str) -> Option<&str> {
        self.modes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, m)| m.as_str())
    }

    fn fixture_reply(&self, key: &str, params: &[(String, String)], n: u32) -> Option<Reply> {
        select_fixture(self.fixtures, key, params, n).map(|p| reply_from_fixture(&p, self.now))
    }

    /// The reply of a failure mode, or `None` for modes that shape the normal flow.
    fn mode_outcome(&self, key: &str, mode: &str, params: &[(String, String)]) -> Option<Outcome> {
        let r = match mode {
            "fail" => Reply::http(
                "HTTP 500 Internal Server Error",
                "UnknownError",
                "mock fail",
            ),
            "unmapped" => Reply::new(1, "", "Error: something the mapping table does not know"),
            "garbage" => Reply::new(0, "not json", ""),
            "silent" => Reply::new(0, "", ""),
            "stopped" => Reply::new(0, "stopped", ""),
            "not-found" => Reply::http(
                "HTTP 404 Not Found",
                "SessionNotFoundError",
                "session not found",
            ),
            "invalid-request" => {
                Reply::http("HTTP 400 Bad Request", "InvalidRequestError", "bad request")
            }
            "409" => Reply::http("HTTP 409 Conflict", "ConflictError", "busy"),
            "synthetic-conflict" => Reply::http(
                "HTTP 409 Conflict",
                "ConflictError",
                "Synthetic input ID conflicts with an existing durable record: msg_x",
            ),
            "404-vanished" => Reply::http("HTTP 404 Not Found", "PermissionNotFoundError", "gone"),
            "503" => Reply::http(
                "HTTP 503 Service Unavailable",
                "ServiceUnavailableError",
                "unavailable",
            ),
            "unauthorized" => Reply::new(1, "", "Error: Server at http://x rejected the password"),
            "unknown-op" => Reply::new(
                1,
                "",
                &unknown_error_stack("Error: Operation not found: nope"),
            ),
            "unknown-error" => Reply::new(
                1,
                "",
                &unknown_error_stack("Error: Missing path parameter: sessionID"),
            ),
            "unreachable" => Reply::new(1, "", UNREACHABLE_STDERR),
            "slow" => return Some(Outcome::Slow),
            "big" => return Some(Outcome::Big),
            "permission" | "form" => {
                // Empty until MOCK_OPENCODE_AFTER_POLLS calls have been served.
                if count_poll(self.state, key) > self.after_polls {
                    Reply::new(0, &pending_row(mode, params).to_string(), "")
                } else {
                    return None;
                }
            }
            other => Reply::new(2, "", &format!("mock-opencode: unknown mode {other}")),
        };
        Some(Outcome::Reply(r))
    }

    fn api(&self, op: &str, params: &[(String, String)], body: Option<&str>) -> Outcome {
        let n = next_seq(self.state, op);
        if let Some(mode) = self.mode_of(op) {
            if let Some(o) = self.mode_outcome(op, mode, params) {
                return o;
            }
        }
        let get = |name: &str| params.iter().any(|(k, _)| k == name);
        let bad =
            |tag: &str, msg: &str| Outcome::Reply(Reply::new(1, &tagged(tag, msg), "HTTP 400"));
        if op == "session.message.list" {
            if params.iter().any(|(k, v)| k == "type" && v == "idle") {
                return bad("InvalidRequestError", "invalid type");
            }
            if get("cursor") && get("order") {
                return bad("InvalidCursorError", "order not allowed with cursor");
            }
        }
        if let Some(body) = body {
            let table = fs::read_to_string(self.fixtures.join("openapi.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|d| OperationTable::from_openapi(&d, TableSource::Live).ok());
            match serde_json::from_str::<Value>(body) {
                Err(_) => return bad("InvalidRequestError", "invalid json body"),
                Ok(v) => {
                    if let Some(prop) = table.and_then(|t| validate_body(&t, op, &v)) {
                        return bad("InvalidRequestError", &format!("missing {prop}"));
                    }
                }
            }
        }
        match self.fixture_reply(op, params, n) {
            Some(r) => Outcome::Reply(r),
            None => Outcome::Reply(Reply::new(
                1,
                &json!({"_tag": "MockMissingFixture", "key": fixture_key(op, params)}).to_string(),
                "",
            )),
        }
    }

    fn service(&self, verb: &str) -> Outcome {
        let key = format!("service.{verb}");
        let n = next_seq(self.state, &key);
        if let Some(mode) = self.mode_of(&key) {
            // `stopped` is meaningful for `service.status` only.
            if mode != "stopped" || verb == "status" {
                if let Some(o) = self.mode_outcome(&key, mode, &[]) {
                    return o;
                }
            }
        }
        if let Some(r) = self.fixture_reply(&key, &[], n) {
            return Outcome::Reply(r);
        }
        let out = if verb == "status" { DEFAULT_URL } else { "" };
        Outcome::Reply(Reply::new(0, out, ""))
    }
}

fn finish(ctx: &Ctx, outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Reply(r) => {
            write_stdout(r.stdout.as_bytes());
            let _ = std::io::stderr().write_all(r.stderr.as_bytes());
            r.exit
        }
        Outcome::Big => {
            let mut bytes = big_array(ctx.big_bytes);
            if is_stdout_pipe() {
                bytes.truncate(PIPE_CAP);
            }
            write_stdout(&bytes);
            0
        }
        Outcome::Slow => {
            spawn_grandchild(ctx.state);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
    }
}

/// Parse `api` arguments after the subcommand: the first positional(s), `--param k=v`,
/// `-d <json>`, `--server <url>`.
fn parse_api(args: &[String]) -> (Vec<String>, Vec<(String, String)>, Option<String>) {
    let (mut pos, mut params, mut body) = (Vec::new(), Vec::new(), None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--param" => {
                if let Some((k, v)) = it.next().and_then(|kv| kv.split_once('=')) {
                    params.push((k.to_string(), v.to_string()));
                }
            }
            "-d" | "--data" => body = it.next().cloned(),
            "--server" => {
                it.next();
            }
            _ => pos.push(a.clone()),
        }
    }
    (pos, params, body)
}

fn append_log(state: &Path, line: &str) {
    let mut text = line.to_string();
    text.push('\n');
    if let Ok(mut f) = OpenOptions::new()
        .append(true)
        .create(true)
        .open(state.join("argv.jsonl"))
    {
        // ONE write_all so concurrent mock processes never interleave.
        let _ = f.write_all(text.as_bytes());
    }
}

pub fn main() {
    let env = |k: &str| std::env::var(k).ok();
    let (Some(state), Some(fixtures)) = (
        env("MOCK_OPENCODE_STATE_DIR"),
        env("MOCK_OPENCODE_FIXTURES"),
    ) else {
        eprintln!("mock-opencode: MOCK_OPENCODE_STATE_DIR and MOCK_OPENCODE_FIXTURES are required");
        std::process::exit(2);
    };
    let (state, fixtures) = (PathBuf::from(state), PathBuf::from(fixtures));
    let _ = fs::create_dir_all(&state);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let kind = if is_stdout_pipe() { "pipe" } else { "file" };
    let vars: Vec<(String, String)> = std::env::vars().collect();
    append_log(&state, &env_presence_line(&argv, &vars, kind));

    let ctx = Ctx {
        fixtures: &fixtures,
        state: &state,
        modes: env("MOCK_OPENCODE_MODE")
            .map(|m| parse_mode(&m))
            .unwrap_or_default(),
        now: now_ms(&env),
        after_polls: env("MOCK_OPENCODE_AFTER_POLLS")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0),
        big_bytes: env("MOCK_OPENCODE_BIG_BYTES")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_BIG),
    };
    let outcome = match argv.first().map(String::as_str) {
        Some("--version") => Outcome::Reply(Reply::new(0, "opencode v2.0.32\n", "")),
        Some("service") => match argv.get(1).map(String::as_str) {
            Some(v @ ("start" | "status" | "stop" | "restart" | "get" | "set" | "unset")) => {
                ctx.service(v)
            }
            _ => Outcome::Reply(Reply::new(2, "", "mock-opencode: unknown service verb")),
        },
        Some("api") => {
            let (pos, params, body) = parse_api(&argv[1..]);
            match pos.as_slice() {
                [m, path] if m.eq_ignore_ascii_case("GET") && path == "/openapi.json" => {
                    match fs::read_to_string(fixtures.join("openapi.json")) {
                        Ok(t) => Outcome::Reply(Reply::new(0, &t, "")),
                        Err(e) => Outcome::Reply(Reply::new(1, "", &format!("mock-opencode: {e}"))),
                    }
                }
                [op] => ctx.api(op, &params, body.as_deref()),
                _ => Outcome::Reply(Reply::new(
                    2,
                    "",
                    "Expected an operation name or an HTTP method and path",
                )),
            }
        }
        _ => Outcome::Reply(Reply::new(2, "", "mock-opencode: unsupported command")),
    };
    std::process::exit(finish(&ctx, outcome));
}
