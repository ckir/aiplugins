# Plugin Footprint: File-Backed Sources and the CI Gate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the resident measurement with the file-backed sources it currently omits, then gate CI on it so a plugin's context cost cannot grow unnoticed.

**Architecture:** `tools/plugin-footprint` already probes a plugin's MCP server and emits a footprint document (spec §5). Part A adds a second source reader — skill and agent frontmatter for the Resident tier, their bodies for Invocation — so the document stops being a lower bound. Part B commits one document per plugin under `docs/footprints/`, then adds a five-layer gate that compares a fresh hermetic byte measurement against the merge base's committed copy.

**Tech Stack:** Rust (no new dependencies; `insta` is already a workspace dev-dependency at `Cargo.toml:52`), `just`, GitHub Actions, `bash` + `jq` for the repo's `scripts/check-*.sh` house style.

**Spec:** `docs/specs/2026-09-03-plugin-footprint.md` — read §3 (the tier model), §4.5 (source acquisition), §5 (the output contract), §6 and §6.2 (the gate and its baseline), and §8 Forks 2, 3, 6 and 7 (decided).

## Global Constraints

- **Claude Code only.** Fork 3. Qwen's `${extensionPath}` / `${/}` manifest and its `QWEN.md` context file are out of scope; do not add a Qwen reader.
- **A failure is never a zero.** A source that cannot be read is an error or an explicitly unmeasured source. It is never `bytes: 0`. This rule has already caught three real defects in this crate; it is the one to hold hardest.
- **Tokens stay absent.** Fork 6(b) makes the exact counter authoritative, and it is not implemented. Every `tokens` field stays `None`. Do not add an estimator.
- **Bytes are the canonical length.** Every byte count goes through `plugin_footprint::canonical::canonical_len` for JSON, or `str::len()` (UTF-8 bytes) for text. Never `chars().count()`.
- **No new dependencies.** If a task seems to need one, stop and report rather than adding it — `cargo deny` gates the tree.
- **Every commit must pass the repo gate:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo nextest run --workspace`. Note `cargo nextest` does NOT apply `-D warnings`; a suite can be green while clippy fails. Run both.

## File Structure

| File | Responsibility |
|---|---|
| `tools/plugin-footprint/src/sources.rs` | **New.** Read a plugin's file-backed sources: frontmatter (Resident) and bodies (Invocation). Pure filesystem reading; no MCP, no process launching. |
| `tools/plugin-footprint/src/document.rs` | **Modify.** Gains an `invocation` tier and folds file sources into `resident`. |
| `tools/plugin-footprint/src/main.rs` | **Modify.** Calls the new reader; gains `--out` to write a document to a path. |
| `tools/plugin-footprint/src/gate.rs` | **New.** The five layers of §6, reading a baseline document rather than measuring it. |
| `tools/plugin-footprint/src/bin/footprint-gate.rs` | **New.** The gate's binary, so CI runs one command. |
| `docs/footprints/<plugin>.json` | **New, generated, committed.** One measurement per plugin. Deliberately NOT inside the plugin directory: `scripts/bundle-plugin.sh` copies that directory into the shipped zip, so a footprint file living there would be downloaded by every user — an absurd cost for a tool that exists to reduce what users download. |
| `docs/footprints/budgets.json` | **New, generated, committed.** The per-plugin thresholds, kept separate from the measurements. §6 rules out hand-maintaining them in the Justfile and suggests the footprint document; a third file is better than either, because a measurement and a policy are different things and regenerating the first must not silently rewrite the second. One small file also makes a threshold change legible in review, which is the whole point of committing it. |
| `Justfile` | **Modify.** New `footprint` recipe; added to the `check:` aggregate at line 75. |
| `.github/workflows/ci.yml` | **Modify.** New single-OS `footprint` job (Fork 7a). |

---

## Part A — §4.5, the file-backed sources

### Task 1: Split frontmatter from body

**Files:**
- Create: `tools/plugin-footprint/src/sources.rs`
- Modify: `tools/plugin-footprint/src/lib.rs` (add `pub mod sources;`)
- Test: in-module `#[cfg(test)] mod tests` in `sources.rs`

**Interfaces:**
- Produces: `pub fn split_frontmatter(text: &str) -> Option<(&str, &str)>` returning `(frontmatter, body)` with the `---` delimiter lines excluded from both, or `None` when the text has no frontmatter block.

- [ ] **Step 1: Write the failing tests**

Create `tools/plugin-footprint/src/sources.rs` with only this test module and a stub:

```rust
//! Reading a plugin's file-backed sources (spec §4.5).

/// Split a `---`-delimited frontmatter block from the body that follows.
pub fn split_frontmatter(_text: &str) -> Option<(&str, &str)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_the_closing_delimiter() {
        let text = "---\nname: doctor\ndescription: d\n---\n\n# Body\ntext\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: doctor\ndescription: d\n");
        assert_eq!(body, "\n# Body\ntext\n");
    }

    #[test]
    fn a_file_without_frontmatter_has_none() {
        // A plain Markdown file is not an error — it simply contributes no
        // frontmatter source. Returning an empty frontmatter instead would
        // report a measurement of zero for something never measured.
        assert!(split_frontmatter("# Just a heading\n").is_none());
    }

    #[test]
    fn an_unterminated_frontmatter_block_is_not_frontmatter() {
        // Treating the whole file as frontmatter would move a body's bytes into
        // the Resident tier, overstating what the host holds on every request.
        assert!(split_frontmatter("---\nname: x\nno closing delimiter\n").is_none());
    }

    #[test]
    fn a_delimiter_inside_the_body_does_not_reopen_the_block() {
        let text = "---\nname: x\n---\nbody\n---\nmore body\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: x\n");
        assert_eq!(body, "body\n---\nmore body\n");
    }

    #[test]
    fn crlf_line_endings_split_the_same_way() {
        // This repository is developed on Windows and `.gitattributes` is not
        // pinning these files, so a checked-out SKILL.md can carry CRLF. A
        // splitter that only recognised "---\n" would find no frontmatter and
        // silently drop every skill from the Resident tier.
        let text = "---\r\nname: x\r\n---\r\nbody\r\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: x\r\n");
        assert_eq!(body, "body\r\n");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p plugin-footprint --lib sources`
Expected: FAIL — the `todo!()` panics.

- [ ] **Step 3: Write the implementation**

Replace the stub:

```rust
/// Split a `---`-delimited frontmatter block from the body that follows.
///
/// Returns `None` when the text does not open with a delimiter, or opens one it
/// never closes. Both cases mean "no frontmatter here", and neither may be
/// reported as an empty frontmatter: a file with no frontmatter contributes no
/// Resident source, which is different from contributing zero bytes.
///
/// Line endings are matched tolerantly. This repository is developed on Windows
/// and does not pin these files in `.gitattributes`, so a checked-out `SKILL.md`
/// can carry CRLF; a splitter keyed on `"---\n"` alone would find no frontmatter
/// and drop every skill out of the Resident tier without failing anything.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let after_open = strip_delimiter_line(text)?;

    let mut offset = 0;
    while offset < after_open.len() {
        let rest = &after_open[offset..];
        if let Some(body) = strip_delimiter_line(rest) {
            return Some((&after_open[..offset], body));
        }
        // Advance to the start of the next line.
        match rest.find('\n') {
            Some(newline) => offset += newline + 1,
            None => break,
        }
    }
    None
}

/// If `text` begins with a `---` line, return what follows it.
fn strip_delimiter_line(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---")?;
    // Accept CRLF, LF, and a delimiter that ends the file.
    if let Some(rest) = rest.strip_prefix("\r\n") {
        return Some(rest);
    }
    if let Some(rest) = rest.strip_prefix('\n') {
        return Some(rest);
    }
    rest.is_empty().then_some(rest)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p plugin-footprint --lib sources`
Expected: PASS, 5 tests.

- [ ] **Step 5: Wire the module in and run the repo gate**

Append to `tools/plugin-footprint/src/lib.rs`:

```rust
pub mod sources;
```

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo nextest run --workspace`
Expected: clippy silent; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add tools/plugin-footprint/src/sources.rs tools/plugin-footprint/src/lib.rs
git commit -m "feat(plugin-footprint): split frontmatter from body"
```

---

### Task 2: Read a plugin's file-backed sources

**Files:**
- Modify: `tools/plugin-footprint/src/sources.rs`
- Test: `tools/plugin-footprint/tests/sources.rs` (create)

**Interfaces:**
- Consumes: `split_frontmatter` from Task 1.
- Produces:
  - `pub struct FileSource { pub kind: &'static str, pub id: String, pub bytes: u64 }`
  - `pub struct FileSources { pub resident: Vec<FileSource>, pub invocation: Vec<FileSource> }`
  - `pub fn read_file_sources(plugin_dir: &Path) -> Result<FileSources, SourceError>`
  - `pub enum SourceError { Read { path: PathBuf, source: std::io::Error } }`

**What counts as what (spec §3), and what does not:**

| Path | Frontmatter → | Body → |
|---|---|---|
| `skills/<name>/SKILL.md` | Resident, `kind: "skill_frontmatter"`, `id: <name>` | Invocation, `kind: "skill_body"`, `id: <name>` |
| `agents/<name>.md` | Resident, `kind: "agent_frontmatter"`, `id: <name>` | Invocation, `kind: "agent_body"`, `id: <name>` |
| `commands/<name>.md` | Resident, `kind: "command_frontmatter"`, `id: <name>` | Invocation, `kind: "command_body"`, `id: <name>` |

Not measured, and each for a stated reason:
- `README.md`, `examples/*.md` — documentation for a human reading the repository. The host never loads them.
- `hooks/hooks.json` — declares commands, not output. §4.5 already rules that hook stdout is not measured, because measuring it means executing contributor-authored code against a synthetic event.
- There is no `commands/` directory and no `CLAUDE.md` in either published plugin today (verified). The command rows above exist so a future one is measured without a code change; do not invent files to match them.

- [ ] **Step 1: Write the failing tests**

Create `tools/plugin-footprint/tests/sources.rs`:

```rust
//! Reading a plugin's file-backed resident and invocation sources (spec §4.5).

use plugin_footprint::sources::read_file_sources;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-sources-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture");
        Self { dir }
    }

    fn write(&self, relative: &str, contents: &str) -> &Self {
        let path = self.dir.join(relative);
        std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create dirs");
        std::fs::write(path, contents).expect("write file");
        self
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

const SKILL: &str = "---\nname: doctor\ndescription: d\n---\n\nbody bytes\n";

#[test]
fn a_skill_contributes_frontmatter_to_resident_and_its_body_to_invocation() {
    let fx = Fixture::new("skill");
    fx.write("skills/doctor/SKILL.md", SKILL);

    let sources = read_file_sources(fx.path()).expect("reads");

    let resident: Vec<_> = sources.resident.iter().map(|s| (s.kind, s.id.as_str())).collect();
    assert_eq!(resident, vec![("skill_frontmatter", "doctor")]);

    let invocation: Vec<_> = sources.invocation.iter().map(|s| (s.kind, s.id.as_str())).collect();
    assert_eq!(invocation, vec![("skill_body", "doctor")]);

    // The two halves together are the whole file, and neither includes the
    // `---` delimiter lines.
    let total: u64 = sources.resident[0].bytes + sources.invocation[0].bytes;
    assert_eq!(total as usize, SKILL.len() - "---\n---\n".len());
}

#[test]
fn an_agent_contributes_both_halves_too() {
    let fx = Fixture::new("agent");
    fx.write("agents/re-analyst.md", "---\nname: re-analyst\n---\nbody\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert_eq!(sources.resident[0].kind, "agent_frontmatter");
    assert_eq!(sources.resident[0].id, "re-analyst");
    assert_eq!(sources.invocation[0].kind, "agent_body");
}

#[test]
fn a_plugin_with_no_skills_or_agents_has_no_file_sources() {
    // Legitimate: an MCP-only plugin. Not an error, and not a failure to read.
    let fx = Fixture::new("bare");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty());
    assert!(sources.invocation.is_empty());
}

#[test]
fn readme_and_examples_are_not_measured() {
    // The host never loads them; they are documentation for whoever reads the
    // repository. Counting them would overstate what a user actually pays.
    let fx = Fixture::new("docs");
    fx.write("README.md", "# readme\n");
    fx.write("examples/x.local.md", "---\nname: x\n---\nbody\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty(), "got {:?}", sources.resident);
    assert!(sources.invocation.is_empty());
}

#[test]
fn a_skill_without_frontmatter_is_all_invocation_and_no_resident() {
    // It contributes no discovery text, so it costs nothing on every request —
    // but its body is still loaded when the skill runs.
    let fx = Fixture::new("nofront");
    fx.write("skills/plain/SKILL.md", "# no frontmatter\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty());
    assert_eq!(sources.invocation.len(), 1);
    assert_eq!(sources.invocation[0].bytes, "# no frontmatter\n".len() as u64);
}

#[test]
fn a_mis_structured_skill_is_an_error_not_a_silent_skip() {
    // `skills/doctor.md` instead of `skills/doctor/SKILL.md`. Appending
    // SKILL.md to it yields a path that does not exist, and skipping on that
    // basis drops a real skill's bytes out of the footprint with nothing
    // failing. A measurement that quietly omits a source is the same false zero
    // the probe layers refuse.
    let fx = Fixture::new("misplaced");
    fx.write("skills/doctor.md", SKILL);

    let err = read_file_sources(fx.path()).expect_err("a mis-structured skill must be loud");

    assert!(
        err.to_string().contains("SKILL.md"),
        "the error must say where the file belongs, got: {err}"
    );
}

#[test]
fn a_skill_directory_with_no_skill_file_is_an_error_too() {
    let fx = Fixture::new("emptyskill");
    fx.write("skills/doctor/notes.md", "# notes
");

    let err = read_file_sources(fx.path()).expect_err("an empty skill directory must be loud");

    assert!(err.to_string().contains("SKILL.md"), "got: {err}");
}

#[test]
fn sources_come_back_sorted_by_id() {
    // The document must not inherit directory iteration order, which differs
    // between filesystems and would churn a committed copy for no reason.
    let fx = Fixture::new("sorted");
    fx.write("skills/zebra/SKILL.md", SKILL);
    fx.write("skills/alpha/SKILL.md", SKILL);

    let sources = read_file_sources(fx.path()).expect("reads");

    let ids: Vec<&str> = sources.resident.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "zebra"]);
}

#[test]
fn bytes_are_utf8_length_not_character_count() {
    let fx = Fixture::new("utf8");
    // 'é' is one character, two UTF-8 bytes.
    fx.write("skills/s/SKILL.md", "---\nname: é\n---\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert_eq!(sources.resident[0].bytes, "name: é\n".len() as u64);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p plugin-footprint --test sources`
Expected: FAIL — `unresolved import plugin_footprint::sources::read_file_sources`.

- [ ] **Step 3: Write the implementation**

Append to `tools/plugin-footprint/src/sources.rs`:

```rust
use std::path::{Path, PathBuf};

/// One file-backed source, already measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSource {
    pub kind: &'static str,
    pub id: String,
    pub bytes: u64,
}

/// A plugin's file-backed sources, split by the tier that pays for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSources {
    pub resident: Vec<FileSource>,
    pub invocation: Vec<FileSource>,
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not where a source belongs; expected {expected}")]
    Malformed {
        path: PathBuf,
        expected: &'static str,
    },
}

/// Where a plugin keeps each kind of file-backed source, and what to call it.
///
/// `README.md` and `examples/` are deliberately absent: the host never loads
/// them, so counting them would overstate what a user actually pays. Hooks are
/// absent because measuring their output means executing contributor-authored
/// code (spec §4.5).
const LAYOUT: &[Layout] = &[
    Layout {
        dir: "skills",
        nested: true,
        resident_kind: "skill_frontmatter",
        invocation_kind: "skill_body",
    },
    Layout {
        dir: "agents",
        nested: false,
        resident_kind: "agent_frontmatter",
        invocation_kind: "agent_body",
    },
    Layout {
        dir: "commands",
        nested: false,
        resident_kind: "command_frontmatter",
        invocation_kind: "command_body",
    },
];

struct Layout {
    dir: &'static str,
    /// `true` for `skills/<id>/SKILL.md`, `false` for `<dir>/<id>.md`.
    nested: bool,
    resident_kind: &'static str,
    invocation_kind: &'static str,
}

/// Read every file-backed source a plugin contributes.
///
/// A plugin with none of these directories is not an error — an MCP-only plugin
/// is perfectly ordinary. A directory that exists but cannot be read IS an
/// error, because reporting no sources for files we failed to read would
/// understate the footprint and quietly pass the gate.
pub fn read_file_sources(plugin_dir: &Path) -> Result<FileSources, SourceError> {
    let mut out = FileSources::default();

    for layout in LAYOUT {
        for (id, path) in entries(&plugin_dir.join(layout.dir), layout)? {
            let text = std::fs::read_to_string(&path)
                .map_err(|source| SourceError::Read { path: path.clone(), source })?;

            let (front, body) = match split_frontmatter(&text) {
                Some((front, body)) => (Some(front), body),
                None => (None, text.as_str()),
            };

            if let Some(front) = front {
                out.resident.push(FileSource {
                    kind: layout.resident_kind,
                    id: id.clone(),
                    bytes: front.len() as u64,
                });
            }
            if !body.is_empty() {
                out.invocation.push(FileSource {
                    kind: layout.invocation_kind,
                    id,
                    bytes: body.len() as u64,
                });
            }
        }
    }

    // Sorted so the document does not inherit directory iteration order, which
    // differs between filesystems and would churn the committed copy.
    out.resident.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    out.invocation.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    Ok(out)
}

/// The `(id, path)` pairs one layout contributes, or nothing if its directory
/// is absent.
fn entries(dir: &Path, layout: &Layout) -> Result<Vec<(String, PathBuf)>, SourceError> {
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(SourceError::Read {
                path: dir.to_path_buf(),
                source,
            })
        }
    };

    let mut found = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|source| SourceError::Read {
            path: dir.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();

        if layout.nested {
            // A mis-structured skill is an ERROR, not a skip. The loader expects
            // `skills/<name>/SKILL.md`; a `.md` file sitting directly in
            // `skills/`, or a directory with no `SKILL.md` inside it, is a skill
            // somebody meant to write. Skipping it drops its bytes from the
            // measurement with nothing failing anywhere — the false zero this
            // whole tool is built to refuse, arriving through the filesystem
            // instead of through a probe.
            let file = entry.path().join("SKILL.md");
            if !file.is_file() {
                return Err(SourceError::Malformed {
                    path: entry.path(),
                    expected: "skills/<name>/SKILL.md",
                });
            }
            found.push((name, file));
        } else if let Some(id) = name.strip_suffix(".md") {
            found.push((id.to_string(), entry.path()));
        }
    }
    Ok(found)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p plugin-footprint --test sources`
Expected: PASS, 9 tests.

- [ ] **Step 5: Run the repo gate**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo nextest run --workspace`
Expected: clippy silent; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add tools/plugin-footprint/src/sources.rs tools/plugin-footprint/tests/sources.rs
git commit -m "feat(plugin-footprint): read skill and agent sources from disk"
```

---

### Task 3: Fold file sources into the document

**Files:**
- Modify: `tools/plugin-footprint/src/document.rs`
- Modify: `tools/plugin-footprint/tests/document.rs`

**Interfaces:**
- Consumes: `FileSources` from Task 2.
- Produces: `build` gains a seventh parameter, `files: &FileSources`; `Tiers` gains `pub invocation: Tier`.

**Note on `build`'s signature:** it reaches seven parameters here. Clippy's
`too_many_arguments` fires at eight, so this is the last one that fits. If an
eighth is ever needed, introduce a `BuildInput` struct rather than raising the
lint threshold.

- [ ] **Step 1: Write the failing tests**

Append to `tools/plugin-footprint/tests/document.rs`:

```rust
use plugin_footprint::sources::{FileSource, FileSources};

fn some_files() -> FileSources {
    FileSources {
        resident: vec![FileSource {
            kind: "skill_frontmatter",
            id: "doctor".to_string(),
            bytes: 100,
        }],
        invocation: vec![FileSource {
            kind: "skill_body",
            id: "doctor".to_string(),
            bytes: 900,
        }],
    }
}

#[test]
fn file_backed_sources_join_the_resident_tier() {
    let doc = build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        Some("0.6.4"),
        0,
        &ok_outcome(),
        &some_files(),
    );
    let value = serde_json::to_value(&doc).expect("serialises");

    let kinds: Vec<&str> = value["tiers"]["resident"]["sources"]
        .as_array()
        .expect("itemised")
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();

    assert!(kinds.contains(&"mcp_tool_schema"));
    assert!(
        kinds.contains(&"skill_frontmatter"),
        "the resident tier must include the file-backed half, got {kinds:?}"
    );
}

#[test]
fn the_invocation_tier_carries_the_bodies() {
    let doc = build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        None,
        0,
        &ok_outcome(),
        &some_files(),
    );
    let value = serde_json::to_value(&doc).expect("serialises");

    assert_eq!(value["tiers"]["invocation"]["bytes"], 900);
    assert_eq!(value["tiers"]["invocation"]["sources"][0]["kind"], "skill_body");
}

#[test]
fn a_failed_probe_still_omits_every_tier_even_with_file_sources_present() {
    // The file sources were read successfully, but the MCP half was not. A
    // document that reported only the half that worked would look like a
    // complete measurement of a much cheaper plugin.
    let failed = outcome(
        Status::Failed("could not launch".to_string()),
        Vec::new(),
        "bin/x",
    );

    let value = serde_json::to_value(build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        None,
        0,
        &failed,
        &some_files(),
    ))
    .expect("serialises");

    assert!(
        value.get("tiers").is_none() || value["tiers"].is_null(),
        "a partial measurement is not a measurement, got: {}",
        value["tiers"]
    );
}
```

Then update every existing `build(` call in that file to pass `&FileSources::default()` as the seventh argument.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p plugin-footprint --test document`
Expected: FAIL — `this function takes 6 arguments but 7 were supplied`.

- [ ] **Step 3: Implement**

In `tools/plugin-footprint/src/document.rs`:

Add the import:

```rust
use crate::sources::{FileSource, FileSources};
```

Change `Tiers`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct Tiers {
    pub resident: Tier,
    pub invocation: Tier,
}
```

Change `build`'s signature and its `tiers` construction:

```rust
pub fn build(
    plugin: &str,
    plugin_dir: &Path,
    tree: Tree,
    plugin_version: Option<&str>,
    measured_at_epoch_secs: i64,
    outcome: &Outcome,
    files: &FileSources,
) -> Document {
```

```rust
    let tiers = matches!(outcome.status, Status::Ok).then(|| Tiers {
        resident: resident_tier(outcome, &files.resident),
        invocation: tier_from_files(&files.invocation),
    });
```

Replace `resident_tier` and add `tier_from_files`:

```rust
/// The resident tier: what the host holds in every request, for the whole
/// session (spec §3). MCP schemas and prompts, plus the frontmatter the host
/// reads to decide what a skill or agent is for.
fn resident_tier(outcome: &Outcome, files: &[FileSource]) -> Tier {
    let mut sources: Vec<Source> = outcome
        .tools
        .iter()
        .map(|tool| source("mcp_tool_schema", tool))
        .chain(
            outcome
                .prompts
                .iter()
                .map(|prompt| source("mcp_prompt", prompt)),
        )
        .chain(files.iter().map(from_file))
        .collect();

    sources.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    total(sources)
}

/// A tier built only from file-backed sources.
fn tier_from_files(files: &[FileSource]) -> Tier {
    let mut sources: Vec<Source> = files.iter().map(from_file).collect();
    sources.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    total(sources)
}

fn total(sources: Vec<Source>) -> Tier {
    Tier {
        bytes: sources.iter().map(|s| s.bytes).sum(),
        tokens: None,
        sources,
    }
}

fn from_file(file: &FileSource) -> Source {
    Source {
        kind: file.kind,
        id: file.id.clone(),
        bytes: file.bytes,
        tokens: None,
    }
}
```

Note the sort key changed from `id` alone to `(kind, id)`: two sources can now
share an id — a skill's frontmatter and its body — and sorting on the id alone
would leave their relative order to the sort's stability rather than to the
data.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p plugin-footprint --test document`
Expected: PASS.

- [ ] **Step 5: Update `main.rs` and re-measure**

In `tools/plugin-footprint/src/main.rs`, add the import and read the sources
before building:

```rust
use plugin_footprint::sources::read_file_sources;
```

```rust
    let files = match read_file_sources(plugin_dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("plugin-footprint: {e}");
            return ExitCode::from(1);
        }
    };
```

and pass `&files` as `build`'s seventh argument.

Run: `cargo build -p plugin-footprint && ./target/debug/plugin-footprint measure claude-code/re-ghidra-mcp-cc`

Expected: `tiers.resident.bytes` is now about **21 631** rather than 18 419, and
`tiers.invocation` is present and non-zero. The exact figure is whatever the
files measure — record it, do not force it to match this number.

- [ ] **Step 6: Run the repo gate and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo nextest run --workspace
git add tools/plugin-footprint
git commit -m "feat(plugin-footprint): count skill and agent frontmatter as resident"
```

---

### Task 4: Stop calling the tier a lower bound

**Files:**
- Modify: `tools/plugin-footprint/src/document.rs` (the `resident_tier` doc comment)
- Modify: `docs/specs/2026-09-03-plugin-footprint.md` (§4.5's closing note)

- [ ] **Step 1: Update the doc comment**

The comment above `resident_tier` currently says the tier is a LOWER BOUND until
§4.5 is implemented. It now is implemented. Replace that paragraph with what
remains unmeasured and why:

```rust
/// Hook output is still not counted, in either tier: measuring it means
/// executing contributor-authored code against a synthetic event (spec §4.5),
/// and that is a price this tool does not pay. A plugin whose hooks emit a large
/// SessionStart preamble is therefore under-reported, which is the one honest
/// gap left in the measurement.
```

- [ ] **Step 2: Update the spec**

In `docs/specs/2026-09-03-plugin-footprint.md`, §4.5's closing paragraph says the
§2 table's Invocation column is illustrative until §4.5 is implemented. Replace
it with the measured figures from Task 3 Step 5, and record that the Resident
tier is no longer a lower bound except for hook output.

- [ ] **Step 3: Commit**

```bash
git add tools/plugin-footprint/src/document.rs docs/specs/2026-09-03-plugin-footprint.md
git commit -m "docs(plugin-footprint): the resident tier is no longer a lower bound"
```

---

## Part B — §6, the gate

### Task 5: Write documents to disk

**Files:**
- Modify: `tools/plugin-footprint/src/main.rs`
- Modify: `Justfile`
- Create: `docs/footprints/rtk-mcp-cc.json`, `docs/footprints/re-ghidra-mcp-cc.json` (generated)

**Interfaces:**
- Produces: `plugin-footprint measure <plugin-dir> --out <path>` writes the canonical document to `<path>` and prints nothing to stdout.

- [ ] **Step 1: Add `--out` handling**

In `main.rs`, replace the argument match:

```rust
    match args.first().map(String::as_str) {
        Some("measure") if args.len() == 2 => measure(Path::new(&args[1]), None),
        Some("measure") if args.len() == 4 && args[2] == "--out" => {
            measure(Path::new(&args[1]), Some(Path::new(&args[3])))
        }
        _ => {
            eprintln!("usage: plugin-footprint measure <plugin-dir> [--out <path>]");
            ExitCode::from(2)
        }
    }
```

and in `measure`, replace the `println!` with:

```rust
    let text = canonical_json(&value);
    match out {
        // A trailing newline so the file is a well-formed text file and git does
        // not report "\ No newline at end of file" on every regeneration.
        Some(path) => {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("plugin-footprint: creating {}: {e}", parent.display());
                    return ExitCode::from(1);
                }
            }
            if let Err(e) = std::fs::write(path, format!("{text}\n")) {
                eprintln!("plugin-footprint: writing {}: {e}", path.display());
                return ExitCode::from(1);
            }
        }
        None => println!("{text}"),
    }
```

Change `measure`'s signature to `fn measure(plugin_dir: &Path, out: Option<&Path>) -> ExitCode`.

- [ ] **Step 2: Add the Justfile recipe**

Insert after the `marketplace:` recipe (around `Justfile:51`):

```just
# Regenerate the committed footprint document for every published plugin.
# Requires the plugin binaries: `just build-rtk-mcp-cc build-re-ghidra-mcp-cc`.
footprint-regen:
    #!/usr/bin/env bash
    set -euo pipefail
    for plugin in $(jq -r '.plugins[].name' .claude-plugin/marketplace.json | tr -d '\r'); do
        cargo run -q -p plugin-footprint -- measure "claude-code/$plugin" \
            --out "docs/footprints/$plugin.json"
        echo "regenerated docs/footprints/$plugin.json"
    done
```

The `tr -d '\r'` is not decoration: the Windows `jq` emits CRLF, and a plugin
name carrying a carriage return makes every path built from it wrong. Every
existing `scripts/check-*.sh` does the same.

- [ ] **Step 3: Seed and ratchet the budgets**

Fork 2 decided a ratchet plus a per-change delta cap, and §6 is explicit that a
static ceiling is NOT a ratchet: if a plugin's footprint drops and the ceiling
stays, the recovered slack is free for a later change to refill, and every build
stays green while the cost returns to where it was.

So `footprint-regen` maintains the thresholds too. Extend the recipe:

```just
        cargo run -q -p plugin-footprint -- ratchet \
            --measured "docs/footprints/$plugin.json" \
            --budgets docs/footprints/budgets.json
```

and add the subcommand to `main.rs`:

```rust
        Some("ratchet") if args.len() == 5 && args[1] == "--measured" && args[3] == "--budgets" => {
            ratchet(Path::new(&args[2]), Path::new(&args[4]))
        }
```

```rust
/// Seed a plugin's thresholds on first sight, and tighten them when a
/// measurement comes in well under the current ceiling.
///
/// The ratchet has HYSTERESIS on purpose, and its guarantee is BOUNDED — state
/// it precisely, because a vague promise here is the kind that quietly stops
/// being true.
///
/// Lowering on every improvement would mean a change saving 5 bytes tightens the
/// ceiling by 5, so reverting it fails a gate that was green a day earlier. The
/// ceiling therefore only follows a measurement that is a further `HEADROOM`
/// below it: it moves when `measured <= current - 2*HEADROOM`, and moves to
/// `measured + HEADROOM`.
///
/// What that buys, exactly: after a lowering, the new ceiling sits `HEADROOM`
/// above the new measurement, so reverting a saving of up to `HEADROOM` bytes
/// still passes. A saving LARGER than `HEADROOM` is banked, and reverting it
/// will fail the ceiling — which is what a ratchet is for, and is not a bug. Do
/// not describe this as "a revert always stays green"; it is "a revert of a
/// saving up to HEADROOM stays green".
fn ratchet(measured_path: &Path, budgets_path: &Path) -> ExitCode {
    const HEADROOM: u64 = 2_000;
    const DELTA: u64 = 500;

    let Ok(text) = std::fs::read_to_string(measured_path) else {
        eprintln!("plugin-footprint: cannot read {}", measured_path.display());
        return ExitCode::from(1);
    };
    let Ok(document) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!("plugin-footprint: {} does not parse", measured_path.display());
        return ExitCode::from(1);
    };

    // A failed measurement must never move a threshold. Its tiers are absent,
    // and treating that as a footprint of zero would ratchet every budget to
    // the floor and fail every subsequent change.
    if document["probe"]["status"].as_str() != Some("ok") {
        eprintln!(
            "plugin-footprint: refusing to ratchet from a probe that did not succeed ({})",
            document["probe"]["status"].as_str().unwrap_or("absent")
        );
        return ExitCode::from(1);
    }

    let plugin = document["plugin"].as_str().unwrap_or_default().to_string();
    let measured = document["tiers"]["resident"]["bytes"].as_u64().unwrap_or(0);

    let mut budgets: serde_json::Value = std::fs::read_to_string(budgets_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let current = budgets[&plugin]["residentBytes"].as_u64();
    let target = measured + HEADROOM;
    let next = match current {
        // Seed: the budget starts where the plugin already is, plus headroom.
        None => target,
        // Tighten only when the measurement is a further headroom below the
        // ceiling — the hysteresis described above.
        Some(current) if target + HEADROOM <= current => target,
        Some(current) => current,
    };

    budgets[&plugin] = serde_json::json!({
        "residentBytes": next,
        "headroomBytes": HEADROOM,
        "deltaBytes": DELTA,
    });

    let text = canonical_json(&budgets);
    if let Err(e) = std::fs::write(budgets_path, format!("{text}\n")) {
        eprintln!("plugin-footprint: writing {}: {e}", budgets_path.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
```

- [ ] **Step 4: Generate and inspect**

Run:
```bash
just build-rtk-mcp-cc
just build-re-ghidra-mcp-cc
just footprint-regen
jq '.tiers.resident.bytes, .probe.status' docs/footprints/re-ghidra-mcp-cc.json
jq . docs/footprints/budgets.json
```

Expected: a status of `"ok"`, a resident byte count, and a budgets file naming
both plugins with `residentBytes` = measured + 2000.

Confirm no file carries an absolute path or a machine-specific string:

```bash
grep -c "Users\|home/" docs/footprints/*.json
```
Expected: `0` for each file.

Then prove the hysteresis holds. Run `just footprint-regen` a second time with
nothing changed and confirm `git diff docs/footprints/budgets.json` is empty — a
ratchet that moved on an unchanged measurement would churn the file on every run.

- [ ] **Step 5: Commit**

```bash
git add tools/plugin-footprint/src/main.rs Justfile docs/footprints
git commit -m "feat(plugin-footprint): commit footprint documents and ratcheted budgets"
```

---

### Task 6: The gate's hermetic comparison

**Files:**
- Create: `tools/plugin-footprint/src/gate.rs`
- Modify: `tools/plugin-footprint/src/lib.rs`
- Test: `tools/plugin-footprint/tests/gate.rs` (create)

**Interfaces:**
- Produces:
  - `pub struct Budget { pub resident_bytes: u64, pub headroom_bytes: u64, pub delta_bytes: u64 }`
  - `pub enum Verdict { Pass, Fail(Vec<String>) }`
  - `pub fn check(measured: &serde_json::Value, baseline: Option<&serde_json::Value>, budget: &Budget) -> Verdict`

`check` takes parsed documents rather than reading files, so every layer is
testable without a filesystem, a git checkout or a built binary.

**The five layers of §6, and where each one lives.**

Four are in `check`. The fifth — freshness — is deliberately NOT, and getting
that wrong is easy: `check` receives the COMMITTED document, so asking it to
verify freshness would have it compare that file against the merge base's copy,
which answers "did this change" and not "is this still true". Freshness is a
question about the world, not about two files, so it is enforced by regenerating
the document and requiring no diff (Task 9). That is also exactly how
`scripts/check-qwen-marketplace.sh` keeps its generated manifest honest.

In `check`, in order — the order is the contract, because an earlier layer's
failure makes every later one meaningless:

1. **Probe assertion** — `probe.status == "ok"` and `probe.toolCount > 0`. Fatal
   and alone: a plugin that measured nothing satisfies every ceiling.
2. **Schema** — the baseline's `schemaVersion` is readable and not newer than
   the measured one.
3. **Budget** — `tiers.resident.bytes` is within the ceiling.
4. **Delta** — growth against the baseline is at most `delta_bytes`.

And outside it, in the Justfile and CI:

5. **Freshness** — `just footprint-regen` followed by `git diff --exit-code
   docs/footprints/`. A committed document that no longer matches a fresh
   measurement fails here, naming the file.

- [ ] **Step 1: Write the failing tests**

Create `tools/plugin-footprint/tests/gate.rs`:

```rust
//! The gate's layers (spec §6). Every case is a pair of parsed documents, so
//! nothing here needs a filesystem, a git checkout or a built binary.

use plugin_footprint::gate::{check, Budget, Verdict};
use serde_json::{json, Value};

fn budget() -> Budget {
    Budget {
        resident_bytes: 20_000,
        headroom_bytes: 2_000,
        delta_bytes: 500,
    }
}

fn doc(status: &str, tools: u64, resident: u64) -> Value {
    json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": status, "toolCount": tools, "binary": "bin/x", "promptCount": 0 },
        "tiers": {
            "resident": {
                "bytes": resident,
                "sources": [{ "kind": "mcp_tool_schema", "id": "t", "bytes": resident }]
            },
            "invocation": { "bytes": 0, "sources": [] }
        }
    })
}

fn failed_doc() -> Value {
    json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": "failed", "detail": "could not launch", "binary": "bin/x",
                   "toolCount": 0, "promptCount": 0 }
    })
}

fn reasons(verdict: Verdict) -> Vec<String> {
    match verdict {
        Verdict::Pass => panic!("expected a failure"),
        Verdict::Fail(reasons) => reasons,
    }
}

#[test]
fn a_clean_measurement_matching_its_baseline_passes() {
    let d = doc("ok", 19, 18_000);
    assert_eq!(check(&d, Some(&d), &budget()), Verdict::Pass);
}

#[test]
fn a_failed_probe_fails_the_gate_before_anything_else_is_considered() {
    // Layer 1 is fatal precisely because every later layer would be reasoning
    // about numbers that were never measured.
    let verdict = reasons(check(&failed_doc(), None, &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("probe")),
        "the probe failure must be named, got {verdict:?}"
    );
    assert_eq!(verdict.len(), 1, "no later layer may also report, got {verdict:?}");
}

#[test]
fn a_probe_reporting_no_tools_at_all_fails() {
    // The false-zero this whole design guards against: a plugin that measured
    // nothing must not satisfy a ceiling by costing nothing.
    let verdict = reasons(check(&doc("ok", 0, 0), None, &budget()));

    assert!(verdict.iter().any(|r| r.contains("no tools")), "{verdict:?}");
}

#[test]
fn a_measurement_over_budget_fails_naming_the_numbers() {
    let over = doc("ok", 19, 25_000);

    let verdict = reasons(check(&over, Some(&over), &budget()));

    let joined = verdict.join(" ");
    assert!(joined.contains("25000"), "actual must be named: {joined}");
    assert!(joined.contains("20000"), "budget must be named: {joined}");
}

#[test]
fn growth_larger_than_the_delta_cap_fails_even_when_under_budget() {
    // 19000 is comfortably under the 20000 ceiling, so only the delta cap can
    // catch it. That is the whole reason the cap exists: the ceiling permits any
    // growth beneath it, including a single jump nobody intended.
    let baseline = doc("ok", 19, 18_000);
    let grown = doc("ok", 19, 19_000);

    let verdict = reasons(check(&grown, Some(&baseline), &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("delta")),
        "a 1000-byte jump exceeds the 500-byte cap, got {verdict:?}"
    );
    assert!(
        !verdict.iter().any(|r| r.contains("budget")),
        "it is under the ceiling; only the delta may fire, got {verdict:?}"
    );
}

#[test]
fn shrinking_is_never_a_delta_failure() {
    // `saturating_sub` matters here: a plugin that got smaller must not read as
    // enormous growth through an underflow.
    let baseline = doc("ok", 19, 18_000);
    let smaller = doc("ok", 19, 12_000);

    assert_eq!(check(&smaller, Some(&baseline), &budget()), Verdict::Pass);
}

#[test]
fn a_baseline_from_a_newer_schema_is_refused_rather_than_guessed_at() {
    let measured = doc("ok", 19, 18_000);
    let mut newer = measured.clone();
    newer["schemaVersion"] = json!(2);

    let verdict = reasons(check(&measured, Some(&newer), &budget()));

    assert!(verdict.iter().any(|r| r.contains("schemaVersion")), "{verdict:?}");
}

#[test]
fn an_older_baseline_schema_is_read_not_refused() {
    // The PR that bumps the version reads a baseline written by the previous
    // one. Refusing that deadlocks the bump: the merge base can only ever carry
    // the older version.
    let mut measured = doc("ok", 19, 18_000);
    measured["schemaVersion"] = json!(2);
    let older = doc("ok", 19, 18_000);

    let verdict = check(&measured, Some(&older), &budget());

    assert_eq!(verdict, Verdict::Pass, "got {verdict:?}");
}

#[test]
fn a_plugin_with_no_baseline_is_measured_but_not_compared() {
    // A newly added plugin has no merge-base document. It must still satisfy the
    // probe and budget layers; only the comparisons are skipped.
    let d = doc("ok", 19, 18_000);
    assert_eq!(check(&d, None, &budget()), Verdict::Pass);

    let over = doc("ok", 19, 25_000);
    assert!(matches!(check(&over, None, &budget()), Verdict::Fail(_)));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p plugin-footprint --test gate`
Expected: FAIL — `unresolved import plugin_footprint::gate`.

- [ ] **Step 3: Implement**

Create `tools/plugin-footprint/src/gate.rs`:

```rust
//! The gate's layers (spec §6).
//!
//! `check` takes two parsed documents and a budget. It reads no files and runs
//! no processes, so every layer is testable without a filesystem, a git
//! checkout or a built binary — and so the layer ORDER, which is the part that
//! matters, is pinned by tests rather than by a comment.
//!
//! The order is a contract. Layer 1 is fatal because every later layer would
//! otherwise be reasoning about numbers that were never measured: a plugin whose
//! binary will not start measures nothing, and nothing satisfies every ceiling.

use serde_json::Value;

/// What this crate can read. A baseline written by a NEWER version is refused
/// rather than guessed at; an older one is read, because the pull request that
/// bumps the version necessarily reads a baseline written by the previous one.
pub const SUPPORTED_SCHEMA: u64 = 1;

#[derive(Debug, Clone)]
pub struct Budget {
    /// The ceiling on `tiers.resident.bytes`.
    pub resident_bytes: u64,
    /// How far below the ceiling a measurement must fall before the ceiling is
    /// lowered to follow it (spec §6). Recorded here so the gate and the
    /// ratchet share one number.
    pub headroom_bytes: u64,
    /// The most one pull request may add.
    pub delta_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Every reason, not just the first — a reviewer fixing one should be able
    /// to see the others in the same run.
    Fail(Vec<String>),
}

pub fn check(measured: &Value, baseline: Option<&Value>, budget: &Budget) -> Verdict {
    // Layer 1: the probe. Fatal, and alone — nothing later is meaningful.
    if let Some(reason) = probe_failed(measured) {
        return Verdict::Fail(vec![reason]);
    }

    let mut reasons = Vec::new();

    // The schema check runs before anything uses the baseline: a baseline this
    // tool cannot read is a baseline it must not compare against.
    let comparable = match baseline {
        Some(baseline) => match schema_of(baseline) {
            Some(version) if version <= schema_of(measured).unwrap_or(SUPPORTED_SCHEMA) => true,
            Some(version) => {
                reasons.push(format!(
                    "the baseline's schemaVersion {version} is newer than this tool understands; \
                     upgrade the tool rather than comparing shapes it cannot read"
                ));
                false
            }
            None => {
                reasons.push("the baseline carries no schemaVersion".to_string());
                false
            }
        },
        None => false,
    };

    let measured_bytes = resident_bytes(measured);

    if comparable {
        let baseline = baseline.expect("comparable implies a baseline");

        // The per-change delta cap. `saturating_sub` so a plugin that got
        // SMALLER does not underflow into apparent enormous growth.
        let growth = measured_bytes.saturating_sub(resident_bytes(baseline));
        if growth > budget.delta_bytes {
            reasons.push(format!(
                "this change adds {growth} resident bytes, over the per-change delta cap of {}",
                budget.delta_bytes
            ));
        }
    }

    // The budget ceiling.
    if measured_bytes > budget.resident_bytes {
        reasons.push(format!(
            "resident footprint {measured_bytes} bytes exceeds the budget of {} bytes",
            budget.resident_bytes
        ));
    }

    if reasons.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail(reasons)
    }
}

fn probe_failed(measured: &Value) -> Option<String> {
    let probe = measured.get("probe")?;
    let status = probe.get("status").and_then(Value::as_str).unwrap_or("absent");
    if status != "ok" {
        let detail = probe
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("no detail recorded");
        return Some(format!("the probe did not succeed ({status}): {detail}"));
    }
    if probe.get("toolCount").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Some(
            "the probe succeeded but reported no tools at all; a measurement of nothing \
             satisfies every ceiling and must not be treated as one"
                .to_string(),
        );
    }
    None
}

fn schema_of(document: &Value) -> Option<u64> {
    document.get("schemaVersion")?.as_u64()
}

fn resident_bytes(document: &Value) -> u64 {
    document
        .get("tiers")
        .and_then(|t| t.get("resident"))
        .and_then(|r| r.get("bytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

```

Add to `tools/plugin-footprint/src/lib.rs`:

```rust
pub mod gate;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p plugin-footprint --test gate`
Expected: PASS, 9 tests.

- [ ] **Step 5: Mutation-check the layer order**

The order is the contract, so prove the test that pins it can fail. Temporarily
move the `probe_failed` check below the budget layer, run
`cargo test -p plugin-footprint --test gate`, and confirm
`a_failed_probe_fails_the_gate_before_anything_else_is_considered` fails. Restore
the order and re-run.

- [ ] **Step 6: Run the repo gate and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo nextest run --workspace
git add tools/plugin-footprint/src/gate.rs tools/plugin-footprint/src/lib.rs tools/plugin-footprint/tests/gate.rs
git commit -m "feat(plugin-footprint): the gate's five layers"
```

---

### Task 7: Snapshot the per-source breakdown

**Files:**
- Create: `tools/plugin-footprint/tests/snapshot.rs`
- Create: `tools/plugin-footprint/tests/snapshots/` (generated by `insta`)

**Why a snapshot as well as a budget:** the budget answers "is it too big"; the
snapshot answers "what changed". A schema edit shows up in review as a readable
diff naming the tool that grew, which a single integer cannot do.

- [ ] **Step 1: Write the test**

Create `tools/plugin-footprint/tests/snapshot.rs`:

```rust
//! A readable diff of what each published plugin is made of (spec §6).
//!
//! The budget says whether a footprint is too big. This says WHAT CHANGED, which
//! is the part a reviewer can act on.
//!
//! Only the per-source breakdown is snapshotted. `measuredAt` moves on every
//! regeneration and `probe.binary` differs by platform, so including either
//! would make this fail for reasons having nothing to do with a footprint.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn breakdown(plugin: &str) -> Option<serde_json::Value> {
    let path = repo_root().join("docs").join("footprints").join(format!("{plugin}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).expect("committed document parses");
    Some(serde_json::json!({
        "resident": document["tiers"]["resident"],
        "invocation": document["tiers"]["invocation"],
    }))
}

#[test]
fn rtk_mcp_cc_breakdown() {
    let Some(value) = breakdown("rtk-mcp-cc") else {
        eprintln!("SKIP: docs/footprints/rtk-mcp-cc.json absent; run `just footprint-regen`");
        return;
    };
    insta::assert_json_snapshot!(value);
}

#[test]
fn re_ghidra_mcp_cc_breakdown() {
    let Some(value) = breakdown("re-ghidra-mcp-cc") else {
        eprintln!("SKIP: docs/footprints/re-ghidra-mcp-cc.json absent; run `just footprint-regen`");
        return;
    };
    insta::assert_json_snapshot!(value);
}
```

- [ ] **Step 2: Accept the snapshots**

Run: `cargo test -p plugin-footprint --test snapshot`
Expected: FAIL — `insta` writes `.snap.new` files.

Review them by eye, confirm they name the tools and skills you expect, then:

Run: `cargo insta accept` (or rename each `.snap.new` to `.snap` if `cargo-insta` is not installed).

Run: `cargo test -p plugin-footprint --test snapshot`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tools/plugin-footprint/tests/snapshot.rs tools/plugin-footprint/tests/snapshots
git commit -m "test(plugin-footprint): snapshot each plugin's source breakdown"
```

---

### Task 8: The gate binary, reading its baseline from the merge base

**Files:**
- Create: `tools/plugin-footprint/src/bin/footprint-gate.rs`
- Modify: `tools/plugin-footprint/Cargo.toml` (no new dependencies; the bin is discovered automatically)

**Interfaces:**
- Consumes: `gate::check`, `gate::Budget` from Task 6.
- Produces: `footprint-gate <base-ref>` — exit 0 on pass, 1 on any failure, 2 on usage.

**Why the baseline comes from git rather than the working tree (spec §6.2):**
CI checks out the pull request. A baseline read from that checkout is authored by
the same change the gate is judging, so a failing change could raise its own
threshold and the diff would look like a routine regeneration. `git show
<base-ref>:<path>` reads the branch being merged INTO.

- [ ] **Step 1: Write the binary**

```rust
//! The footprint gate (spec §6), as one command for CI to run.
//!
//! Usage: `footprint-gate <base-ref>` — for example `footprint-gate origin/main`.

use plugin_footprint::gate::{check, Budget, Verdict};
use std::process::{Command, ExitCode};

/// Where the thresholds live. Read from the BASE REF, never from the working
/// tree (spec §6.2): CI checks out the pull request, so a budget read from that
/// checkout is authored by the very change the gate is judging, and a failing
/// change could raise its own ceiling with a diff that looks like a routine
/// regeneration.
const BUDGETS_PATH: &str = "docs/footprints/budgets.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(base_ref) = args.first() else {
        eprintln!("usage: footprint-gate <base-ref>");
        return ExitCode::from(2);
    };

    // The plugin list comes from the marketplace manifest, the same source
    // `just smoke` and the bundle-smoke job iterate. Reading it from the working
    // tree is deliberate: a change that ADDS a plugin must have it measured.
    let plugins = match published_plugins() {
        Ok(plugins) => plugins,
        Err(e) => {
            eprintln!("footprint-gate: {e}");
            return ExitCode::from(1);
        }
    };

    // Budgets from the base ref. Absent on the very first run, which is not a
    // failure — every plugin is then new and only the probe layer applies.
    let budgets = show(base_ref, BUDGETS_PATH).unwrap_or_else(|| serde_json::json!({}));

    let mut failed = false;
    for plugin in &plugins {
        let budget = match budget_for(&budgets, plugin) {
            Some(budget) => budget,
            None => {
                println!(
                    "footprint-gate: {plugin} has no budget at {base_ref} yet; \
                     measuring without a ceiling"
                );
                Budget {
                    resident_bytes: u64::MAX,
                    headroom_bytes: 0,
                    delta_bytes: u64::MAX,
                }
            }
        };
        let path = format!("docs/footprints/{plugin}.json");

        let measured = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    eprintln!("footprint-gate: {path} does not parse: {e}");
                    failed = true;
                    continue;
                }
            },
            Err(e) => {
                eprintln!(
                    "footprint-gate: {path} is missing ({e}). Run `just footprint-regen` and \
                     commit the result."
                );
                failed = true;
                continue;
            }
        };

        // Absent for a plugin added by this change, which is not a failure —
        // there is simply nothing to compare against yet.
        let baseline = show(base_ref, &path);

        match check(&measured, baseline.as_ref(), &budget) {
            Verdict::Pass => println!("footprint-gate: {plugin} ok"),
            Verdict::Fail(reasons) => {
                failed = true;
                eprintln!("footprint-gate: {plugin} FAILED");
                for reason in reasons {
                    eprintln!("  - {reason}");
                }
                report_growth(&measured, baseline.as_ref());
            }
        }
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// The plugins the marketplace actually publishes.
///
/// The same iteration source `just smoke` and the `bundle-smoke` CI job use, so
/// the gate cannot drift from what ships. Note the interaction with §6.2's
/// bypass: a change that removes a plugin from this manifest removes it from
/// the gate. `scripts/check-marketplace.sh` is what keeps the manifest honest
/// about the plugins present, and it already runs in CI.
fn published_plugins() -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(".claude-plugin/marketplace.json")
        .map_err(|e| format!("reading the marketplace manifest: {e}"))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parsing the marketplace manifest: {e}"))?;
    Ok(manifest["plugins"]
        .as_array()
        .ok_or("the marketplace manifest declares no plugins array")?
        .iter()
        .filter_map(|p| p["name"].as_str().map(str::to_string))
        .collect())
}

fn budget_for(budgets: &serde_json::Value, plugin: &str) -> Option<Budget> {
    let entry = budgets.get(plugin)?;
    Some(Budget {
        resident_bytes: entry.get("residentBytes")?.as_u64()?,
        headroom_bytes: entry.get("headroomBytes")?.as_u64()?,
        delta_bytes: entry.get("deltaBytes")?.as_u64()?,
    })
}

/// Read a path as it exists at `base_ref`, or `None` when it is not there.
fn show(base_ref: &str, path: &str) -> Option<serde_json::Value> {
    let output = Command::new("git")
        .args(["show", &format!("{base_ref}:{path}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

/// Name the sources that grew most, so a red build says what to look at.
///
/// The JSON is the interface for tools; a CI log is the interface for a person,
/// and "over budget" without a culprit is merely red.
fn report_growth(measured: &serde_json::Value, baseline: Option<&serde_json::Value>) {
    let sources = |doc: &serde_json::Value| -> Vec<(String, u64)> {
        doc["tiers"]["resident"]["sources"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|s| {
                        (
                            format!(
                                "{}/{}",
                                s["kind"].as_str().unwrap_or("?"),
                                s["id"].as_str().unwrap_or("?")
                            ),
                            s["bytes"].as_u64().unwrap_or(0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let now = sources(measured);
    let before: std::collections::BTreeMap<String, u64> =
        baseline.map(|b| sources(b).into_iter().collect()).unwrap_or_default();

    let mut grown: Vec<(String, i64)> = now
        .iter()
        .map(|(id, bytes)| {
            let was = before.get(id).copied().unwrap_or(0);
            (id.clone(), *bytes as i64 - was as i64)
        })
        .filter(|(_, delta)| *delta != 0)
        .collect();
    grown.sort_by_key(|(_, delta)| -*delta);

    if grown.is_empty() {
        return;
    }
    eprintln!("  largest changes:");
    for (id, delta) in grown.iter().take(3) {
        eprintln!("    {id}: {delta:+} bytes");
    }
}
```

- [ ] **Step 2: Confirm the budgets came from measurement, not from taste**

Task 5 already generated them. Check they are measured + headroom and nothing
more:

```bash
jq -r '"\(.plugin): \(.tiers.resident.bytes)"' docs/footprints/*.json
jq . docs/footprints/budgets.json
```

Every `residentBytes` should equal that plugin's measured bytes plus 2000. Do
not round any of them to a pleasing number — the headroom is the allowance and
the measurement is the fact, and a hand-rounded ceiling is the beginning of a
budget nobody can explain.

- [ ] **Step 3: Verify it passes on a clean tree, and fails when it should**

Run: `cargo run -q -p plugin-footprint --bin footprint-gate -- HEAD`
Expected: `ok` for both plugins, exit 0.

Then prove it bites. Temporarily raise a byte count in
`docs/footprints/re-ghidra-mcp-cc.json` by 1000, re-run, and confirm it reports
BOTH a stale document and a delta-cap failure. Restore the file with
`git checkout -- docs/footprints/re-ghidra-mcp-cc.json`.

- [ ] **Step 4: Commit**

```bash
git add tools/plugin-footprint/src/bin/footprint-gate.rs
git commit -m "feat(plugin-footprint): the gate binary, baselined on the merge base"
```

---

### Task 9: Wire the gate into `just` and CI

**Files:**
- Modify: `Justfile:75` (the `check:` aggregate)
- Modify: `.github/workflows/ci.yml`

**The important fact, verified:** `.github/workflows/ci.yml` never invokes
`just`. It hand-enumerates every step. Adding the gate to the `check:` aggregate
alone would gate a developer who remembers to run it and nothing else — CI would
not run it at all. Both edits are required.

- [ ] **Step 1: Add the Justfile recipe and extend the aggregate**

Insert after the `footprint-regen` recipe from Task 5:

```just
# Verify each published plugin's committed footprint against a fresh measurement,
# then against the thresholds. Requires the plugin binaries; see `footprint-regen`.
footprint:
    #!/usr/bin/env bash
    set -euo pipefail
    # Freshness (spec §6). Regenerating and requiring no diff is what makes the
    # committed document a claim about the world rather than a file that agrees
    # with itself. `check-qwen-marketplace.sh` keeps its generated manifest
    # honest exactly this way, on the stated reasoning that nothing fails when a
    # copy goes stale — it just advertises the wrong thing.
    just footprint-regen
    if ! git diff --quiet -- docs/footprints/; then
        echo "ERROR: the committed footprint documents are stale." >&2
        echo "A fresh measurement disagrees with what is committed:" >&2
        git --no-pager diff --stat -- docs/footprints/ >&2
        echo "Run 'just footprint-regen' and commit the result." >&2
        exit 1
    fi
    # `main`, NOT `HEAD`. Against HEAD the baseline is the developer's own last
    # commit, so once they have run `footprint-regen` and committed it to satisfy
    # the freshness check above, the measured delta is zero BY CONSTRUCTION and
    # the delta cap can never fire locally. `just check` would report green on
    # exactly the change the cap exists to catch, and CI would be the first to
    # say otherwise. Comparing against `main` is the local analogue of what CI
    # does against the base branch.
    cargo run -q -p plugin-footprint --bin footprint-gate -- \
        "$(git rev-parse --verify --quiet main >/dev/null && echo main || echo HEAD)"
```

If the checkout has no local `main` — a shallow or single-branch clone — this
falls back to `HEAD`, and the delta cap is then vacuous locally. That is a real
gap, and it is why the same check runs in CI against the base branch, where the
history is guaranteed by `fetch-depth: 0`. Do not let the local recipe be the
only place this is checked.

Change line 75 from:

```just
check: fmt lint test deny spellcheck wiring marketplace dispatch smoke
```

to:

```just
check: fmt lint test deny spellcheck wiring marketplace dispatch smoke footprint
```

- [ ] **Step 2: Add the CI job**

Insert into `.github/workflows/ci.yml` after the `wiring` job (which ends at
line 103, before `bundle-smoke`):

```yaml
  # Fork 7a: one job, one OS. A footprint is a property of the schema, not of the
  # platform, so a matrix would triple the cost to answer the same question — and
  # it would make "the footprint" an OS-indexed value.
  #
  # This is the only job that needs the plugin binaries staged where the
  # manifests point, so it runs `just build-*` first. `cargo build` alone is not
  # enough: it writes to `target/`, while `.mcp.json` names
  # `${CLAUDE_PLUGIN_ROOT}/bin/<name>`, and it is the `just` recipes that copy
  # them there.
  footprint:
    name: Plugin Footprint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          # The gate compares against the merge base, so the history it needs
          # must actually be in the checkout.
          fetch-depth: 0

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      # `taiki-e/install-action` is the family this workflow already uses for
      # nextest and cargo-deny; keeping to it avoids introducing a second way of
      # putting a tool on PATH.
      - name: Install just
        uses: taiki-e/install-action@just

      - name: Build plugin binaries
        run: |
          just build-rtk-mcp-cc
          just build-re-ghidra-mcp-cc

      # Freshness: a committed document that no longer matches a fresh
      # measurement is caught here, before any threshold is consulted.
      - name: Verify the committed footprints are not stale
        run: |
          just footprint-regen
          if ! git diff --quiet -- docs/footprints/; then
            echo "::error::Committed footprint documents are stale. Run 'just footprint-regen' and commit."
            git --no-pager diff -- docs/footprints/
            exit 1
          fi

      # Thresholds, read from the branch being merged INTO (spec §6.2) so a
      # change cannot raise the ceiling it is judged against.
      - name: Check footprints against the merge base
        run: |
          cargo run -q -p plugin-footprint --bin footprint-gate -- \
            "origin/${{ github.base_ref || 'main' }}"
```

- [ ] **Step 3: Verify the whole aggregate still passes**

Run: `just check`
Expected: every step green, `footprint` included.

- [ ] **Step 4: Verify the CI job's assumptions locally**

The job depends on three things this repository does not otherwise rely on.
Check each rather than trusting the YAML:

```bash
# 1. `just build-*` stages binaries where the manifests point.
just build-rtk-mcp-cc && ls claude-code/rtk-mcp-cc/bin/

# 2. The gate can read a baseline out of git rather than the working tree.
git show HEAD:docs/footprints/rtk-mcp-cc.json | jq -r .plugin

# 3. A shallow checkout would break the merge-base read; confirm the ref
#    resolves before relying on it.
git rev-parse --verify HEAD >/dev/null && echo "base ref resolves"
```

- [ ] **Step 5: Commit**

```bash
git add Justfile .github/workflows/ci.yml
git commit -m "ci: gate every pull request on plugin footprint"
```

---

## Review record

**Solo pass**, before escalation. Three findings, all folded: a static ceiling
where Fork 2 decided a ratchet; budgets as a `const` in the binary where §6 rules
out hand-maintaining them; and — the one that mattered — a gate that read the
COMMITTED document as its measurement, so the freshness layer would have compared
that file against the merge base's copy and answered "did this change" rather
than "is this still true".

**Adversarial panel, round 1 (escalation), 2026-09-03.** Seats: Axiom Breaker,
Cascade Analyst, Mechanism Gamer (self-seated); the rest dropped as untriggered.
Two findings folded, one rejected on measurement, one recorded below.

- FOLDED: a mis-structured skill — `skills/doctor.md` rather than
  `skills/doctor/SKILL.md` — was silently skipped, dropping a real source from
  the measurement with nothing failing. Now an error (Task 2), with tests.
- FOLDED: `just check` ran the gate against `HEAD`, so once a developer
  committed the regenerated document the local delta was zero by construction
  and the delta cap could never fire. Local runs now compare against `main`
  (Task 9), and the remaining gap on a single-branch clone is named there.
- REJECTED, measured: that the gate fails to cross-check the manifest's plugin
  list against the directories on disk, letting a change hide a plugin by
  delisting it. That cross-check already exists and already runs in CI —
  `scripts/check-marketplace.sh:112-121` iterates `claude-code/*/` and fails
  with "exists in claude-code/ but is not listed in $manifest", reached through
  ci.yml's `wiring` job.
- RECORDED, not reachable in this plan: the budget ceiling is checked
  unconditionally, so a future `schemaVersion` bump that enlarges the
  measurement would be judged against the merge base's smaller ceiling and
  deadlock. It cannot arise here — §4.5 lands FIRST precisely so that no
  baseline reset is needed, and `SUPPORTED_SCHEMA` stays 1 throughout. Whoever
  bumps the schema must decide then whether the ceiling is re-seeded or the
  bump is a two-step change; do not add a version-boundary bypass now, because
  a gate that skips its ceiling across a version change is a gate any bloated
  change can walk through by bumping a number.

## What this plan does NOT cover, and why

**§7, the published figure, and Fork 6(b)'s exact oracle.** Deliberately absent.
The published figure is authoritative only if it carries exact token counts, and
the exact counter does not exist yet — planning its integration now would mean
writing line-level steps against absent code, which is the failure the
plan-versus-spec discipline exists to prevent. §7 also needs a decision this plan
does not make: where the release-time regeneration runs, and how a contributor
without an API key gets their pull request merged.

Write that plan once the oracle lands. At that point §7's own PR-time half —
verifying a README region against the committed documents — is a short piece of
work that needs no oracle at all, and `docs/footprints/*.json` from Task 5 is
already the input it reads.

**Hook output.** Still unmeasured, in both tiers, and the one honest gap left in
the measurement after Task 4. Measuring it means executing contributor-authored
code against a synthetic event (§4.5). If it is ever wanted, it needs its own
decision about that, not an implementation task.
