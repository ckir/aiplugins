//! Shared fixture helpers for the gated in-process integration tests (crucible.rs, respawn.rs).
//! All live-worker tests are gated on GHIDRA_MCP_E2E=1 + the fixture env vars.
// Each test binary includes this module via `mod common;` but uses a DIFFERENT subset of the helpers
// (e.g. the write-suite helpers are used only by writenav.rs), so per-binary dead_code is expected here.
#![allow(dead_code)]

use ghidra_ipc::error::{ErrorCode, ErrorEnvelope};
use ghidra_mcp::config::RawConfig;
use ghidra_mcp::paths::versioned_script_dir;
use ghidra_mcp::state::ServerState;
use std::sync::Arc;

pub fn env(n: &str) -> Option<String> {
    std::env::var(n).ok().filter(|s| !s.is_empty())
}

pub fn enabled() -> bool {
    env("GHIDRA_MCP_E2E").as_deref() == Some("1")
}

/// Build ServerState from the fixture env (same vars as boot_e2e.rs). `bootstrap_program` is the BARE
/// leaf of the fixture program VFS path; `bootstrap_program_path` is the full `/`-path.
pub fn fixture_state() -> Arc<ServerState> {
    fixture_state_tuned(|_| {})
}

/// `fixture_state()` with a chance to adjust the resolved config first.
///
/// Exists so a test can pin a TIMING property instead of inheriting one from
/// whatever machine it runs on. `wait_until_ready` only reports WORKER_WARMING
/// once a boot outruns `warming_deadline` (8 s by default), so asserting that a
/// cold call sees WORKER_WARMING is really asserting "this machine takes more
/// than 8 s to boot a JVM" — true on a developer laptop, intermittently false on
/// a CI runner. Shrinking the deadline makes the same assertion deterministic
/// everywhere.
pub fn fixture_state_tuned(
    tune: impl FnOnce(&mut ghidra_mcp::config::ServerConfig),
) -> Arc<ServerState> {
    let program = env("GHIDRA_MCP_FIXTURE_PROGRAM")
        .expect("GHIDRA_MCP_FIXTURE_PROGRAM (VFS path e.g. /add.exe)");
    let bare = program.rsplit('/').next().unwrap().to_string();
    let raw = RawConfig {
        ghidra_install_dir: env("GHIDRA_INSTALL_DIR"),
        project_dir: env("GHIDRA_MCP_FIXTURE_PROJECT_DIR"),
        project_name: env("GHIDRA_MCP_FIXTURE_PROJECT_NAME"),
        bootstrap_program: Some(bare),
        bootstrap_program_path: Some(program),
        max_heap: None,
    };
    let mut cfg = raw.resolve().expect("fixture config resolves");
    tune(&mut cfg);
    Arc::new(ServerState::new(cfg, versioned_script_dir()))
}

/// Poll a call until the background warmup boot is Ready (WORKER_WARMING → retry). Bounds the wait so a
/// stuck boot fails the test instead of hanging.
pub async fn call_when_warm(
    state: &Arc<ServerState>,
    tool: &str,
    sel: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ErrorEnvelope> {
    for _ in 0..60 {
        match ghidra_mcp::execute::call_worker(state, tool, sel, method, params.clone()).await {
            Err(e) if e.error.code == ErrorCode::WorkerWarming => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            other => return other.map(|(v, _canon)| v),
        }
    }
    panic!("worker never warmed within 60s");
}

/// RAII guard: deletes the ephemeral fixture copy on drop so the write suite doesn't leak ~5 project
/// copies per run (agy plan-review R3/R4). Use `.path` to boot/inspect the copy.
///
/// DROP-ORDER IS LOAD-BEARING (agy R4): the guard must drop AFTER the `ServerState` that owns the JVM,
/// or `remove_dir_all` races a live Ghidra handle and Windows denies it. `fixture_state_ephemeral`
/// returns `(guard, state)` and every test binds `let (dir, state) = …` — Rust drops locals in reverse
/// declaration order, so `state` (JVM killed via JobObject) drops BEFORE `dir` (cleanup). Even so the
/// JobObject kill is async, so `drop` spin-retries to absorb the few-ms handle-release lag on Windows.
pub struct EphemeralFixture {
    pub path: std::path::PathBuf,
}
impl Drop for EphemeralFixture {
    fn drop(&mut self) {
        // Windows keeps the Ghidra .lock / program-DB handles open for a few ms after the JobObject kill
        // and denies deletion (ERROR_SHARING_VIOLATION). Spin-retry up to ~2s so cleanup actually happens
        // instead of silently orphaning the copy (a plain best-effort `.ok()` leaves the R3 leak unfixed).
        for _ in 0..40 {
            if !self.path.exists() || std::fs::remove_dir_all(&self.path).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50)); // sync Drop context — std sleep, not tokio
        }
    }
}

/// Copy the fixture project dir to a fresh temp dir and return `(guard, state)` — guard FIRST so it drops
/// LAST (see the drop-order note above). Write e2e MUST use this (spec §11) so it never mutates the shared
/// read-suite fixture. Booting a SECOND state against the SAME `guard.path` (drop the first state first)
/// is the hard-restart durability oracle (§11).
pub fn fixture_state_ephemeral() -> (EphemeralFixture, Arc<ServerState>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0); // unique even if two tests share a PID (non-nextest runner)
    let src = env("GHIDRA_MCP_FIXTURE_PROJECT_DIR").expect("GHIDRA_MCP_FIXTURE_PROJECT_DIR");
    let uniq = format!(
        "{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let dst = std::env::temp_dir().join(format!("ghidra-mcp-w-{uniq}"));
    copy_dir_all(std::path::Path::new(&src), &dst).expect("copy fixture project");
    let state = fixture_state_at(&dst);
    (EphemeralFixture { path: dst }, state)
}

/// Like fixture_state() but with GHIDRA_MCP_FIXTURE_PROJECT_DIR overridden to `dir` (boots against an
/// already-mutated ephemeral copy — the second leg of the hard-restart oracle). CAPTURE + RESTORE the
/// original value (agy plan-review): set_var is process-global, so leaving it mutated would make the NEXT
/// fixture_state_ephemeral() copy this temp dir instead of the pristine source, cascading corruption
/// under -j1.
pub fn fixture_state_at(dir: &std::path::Path) -> Arc<ServerState> {
    let orig =
        std::env::var("GHIDRA_MCP_FIXTURE_PROJECT_DIR").expect("GHIDRA_MCP_FIXTURE_PROJECT_DIR");
    std::env::set_var("GHIDRA_MCP_FIXTURE_PROJECT_DIR", dir);
    let state = fixture_state(); // reads the env var during boot
    std::env::set_var("GHIDRA_MCP_FIXTURE_PROJECT_DIR", orig); // restore before returning
    state
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let e = entry?;
        let p = e.path();
        let d = dst.join(e.file_name());
        if p.file_name()
            .is_some_and(|n| n.to_string_lossy().contains(".lock"))
        {
            continue;
        } // never copy a lock
        if p.is_dir() {
            copy_dir_all(&p, &d)?;
        } else {
            std::fs::copy(&p, &d)?;
        }
    }
    Ok(())
}

// ── Fixture address discovery ──────────────────────────────────────────
//
// These addresses used to be hardcoded constants, "discovered against the
// ghidra-mcp-fix build". That made them properties of ONE historical binary:
// a different compiler lays the data out differently, and the suite then fails
// in its SETUP with an error that describes the fixture rather than the code
// under test. It cost a full CI cycle to work out that
// `set_datatype ... INVALID_PARAMS: address 140075028 is inside a defined data
// unit starting at 1400742a0` meant "your fixture moved", not "set_datatype is
// broken".
//
// So derive them with the same tools the suite is testing. The derivation is
// exactly the one the old comments documented by hand — this just runs it.

/// Addresses the write suites need, resolved against the live program.
#[derive(Debug, Clone)]
pub struct FixtureAddrs {
    /// The `g_rect` global — a data address, used as the NOT_A_FUNCTION oracle.
    pub rect: String,
    /// The `https://example.test/beacon` string literal `g_banner` points to.
    pub string_lit: String,
    /// Start of an undefined gap in writable data, with at least 8 free bytes.
    pub neigh: String,
    /// `neigh + 4` — the neighbour whose definition must block an 8-byte type at `neigh`.
    pub neigh_next: String,
    /// `neigh + 1` — an offcut, i.e. an address mid-unit once a 4-byte type sits at `neigh`.
    pub neigh_offcut: String,
}

/// Discovered once per test process. Every test copies the SAME source project,
/// so the addresses are identical across the ephemeral copies — and discovery
/// costs a handful of RPCs, which is not something to repeat 12 times.
static FIXTURE_ADDRS: tokio::sync::OnceCell<FixtureAddrs> = tokio::sync::OnceCell::const_new();

/// Resolve the fixture's addresses. Call at the START of a test, before it
/// defines anything — discovery needs the pristine layout.
pub async fn fixture_addrs(state: &Arc<ServerState>) -> &'static FixtureAddrs {
    FIXTURE_ADDRS
        .get_or_init(|| async { discover_addrs(state).await })
        .await
}

async fn discover_addrs(state: &Arc<ServerState>) -> FixtureAddrs {
    let rect = data_item_address(state, "g_rect").await;
    let string_lit = banner_string_address(state).await;
    let neigh = undefined_gap_address(state, &rect).await;
    FixtureAddrs {
        neigh_next: offset_address(&neigh, 4),
        neigh_offcut: offset_address(&neigh, 1),
        rect,
        string_lit,
        neigh,
    }
}

/// Address of a named global, via `list_data_items`.
async fn data_item_address(state: &Arc<ServerState>, name: &str) -> String {
    let v = call_when_warm(
        state,
        "list_data_items",
        "",
        "list_data_items",
        serde_json::json!({ "filter": name }),
    )
    .await
    .unwrap_or_else(|e| panic!("list_data_items(filter={name}) failed: {e:?}"));
    v["data"]
        .as_array()
        .unwrap_or_else(|| panic!("list_data_items returned no `data` array: {v:?}"))
        .iter()
        .find(|d| d["name"].as_str() == Some(name))
        .and_then(|d| d["address"].as_str())
        .unwrap_or_else(|| {
            panic!("no data item named `{name}` in the fixture — is the project analyzed? {v:?}")
        })
        .to_string()
}

/// Address of the banner string literal, via `list_strings`.
async fn banner_string_address(state: &Arc<ServerState>) -> String {
    let v = call_when_warm(
        state,
        "list_strings",
        "",
        "list_strings",
        serde_json::json!({ "filter": "https?://.*", "regex": true }),
    )
    .await
    .expect("list_strings failed");
    v["strings"]
        .as_array()
        .unwrap_or_else(|| panic!("list_strings returned no `strings` array: {v:?}"))
        .iter()
        .find(|s| {
            s["value"]
                .as_str()
                .is_some_and(|t| t.contains("example.test"))
        })
        .and_then(|s| s["address"].as_str())
        .unwrap_or_else(|| panic!("the fixture's banner string literal is missing: {v:?}"))
        .to_string()
}

/// Find an 8-byte run of UNDEFINED bytes in the writable data the globals live in.
///
/// Anchored on `g_rect` and scanning forward rather than sweeping the whole
/// program: the fixture's globals cluster together, and the gap the old constant
/// pointed at was 0x28 past `g_rect`. `describe_address` omits `data_type`
/// entirely for an undefined address — that absence is the oracle, and it is the
/// same one the original hand-derivation used.
async fn undefined_gap_address(state: &Arc<ServerState>, anchor: &str) -> String {
    const STRIDE: u64 = 8; // candidates stay 8-aligned so an 8-byte type could fit
    const MAX_STEPS: u64 = 128; // 1 KiB past the anchor; the gap sat ~0x28 in
    for step in 0..MAX_STEPS {
        let candidate = offset_address(anchor, step * STRIDE);
        let next = offset_address(&candidate, 4);
        if is_undefined(state, &candidate).await && is_undefined(state, &next).await {
            return candidate;
        }
    }
    panic!(
        "no 8-byte undefined gap within {} bytes of {anchor}. The fixture's data layout changed \
         enough that the write suite has nowhere neutral to scribble; rebuild the fixture, or \
         widen the scan.",
        MAX_STEPS * STRIDE
    );
}

/// True when `address` is mapped but carries no defined data unit.
async fn is_undefined(state: &Arc<ServerState>, address: &str) -> bool {
    let v = call_when_warm(
        state,
        "describe_address",
        address,
        "describe_address",
        serde_json::json!({ "address": address }),
    )
    .await
    .unwrap_or_else(|e| panic!("describe_address({address}) failed: {e:?}"));
    // Must be inside the image: an unmapped address is "undefined" in the sense
    // of having no data_type, but writing there is a different error entirely.
    v["mapped"].as_bool() == Some(true) && v.get("data_type").is_none_or(|t| t.is_null())
}

/// Add a byte offset to a `ram:<hex>` address, preserving the space prefix.
///
/// Addresses come back as `ram:140075028`; the arithmetic the old constants did
/// in comments (`// D_NEIGH + 4`) has to happen for real now.
pub fn offset_address(address: &str, delta: u64) -> String {
    let (space, hex) = address
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("address `{address}` has no `<space>:` prefix"));
    let base = u64::from_str_radix(hex, 16)
        .unwrap_or_else(|e| panic!("address `{address}` has a non-hex offset: {e}"));
    format!("{space}:{:x}", base + delta)
}

#[cfg(test)]
mod addr_tests {
    use super::offset_address;

    #[test]
    fn offsets_preserve_the_space_prefix_and_stay_hex() {
        assert_eq!(offset_address("ram:140075028", 4), "ram:14007502c");
        assert_eq!(offset_address("ram:140075028", 1), "ram:140075029");
        assert_eq!(offset_address("ram:140075028", 0), "ram:140075028");
    }

    /// Carrying across a nibble boundary is where a naive string edit would break.
    #[test]
    fn offsets_carry_correctly() {
        assert_eq!(offset_address("ram:1400750ff", 1), "ram:140075100");
        assert_eq!(offset_address("ram:14007502c", 4), "ram:140075030");
    }
}
