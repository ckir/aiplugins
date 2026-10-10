//! Shared logic for the `claude-example` Claude Code plugin.
//!
//! Everything in this crate root is pure and unit-testable: the binaries in
//! `src/bin/` are thin shells that read the outside world (stdin, the file
//! system) and hand the bytes to the functions below. That split is the point
//! of the template — protocol plumbing is awkward to test, so keep it small and
//! keep the decisions here.

pub mod hook;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Markers ────────────────────────────────────────────────────────────

/// The kinds of code marker this plugin understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MarkerKind {
    Todo,
    Fixme,
    Hack,
}

impl MarkerKind {
    /// Every kind, in scan priority order.
    pub const ALL: [MarkerKind; 3] = [MarkerKind::Todo, MarkerKind::Fixme, MarkerKind::Hack];

    /// The literal keyword as it appears in source text.
    pub const fn keyword(self) -> &'static str {
        match self {
            MarkerKind::Todo => "TODO",
            MarkerKind::Fixme => "FIXME",
            MarkerKind::Hack => "HACK",
        }
    }

    /// Parse a kind from its keyword, case-insensitively.
    pub fn from_keyword(s: &str) -> Option<Self> {
        MarkerKind::ALL
            .into_iter()
            .find(|k| k.keyword().eq_ignore_ascii_case(s.trim()))
    }
}

/// One marker found in a line of source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub kind: MarkerKind,
    /// The owner from `TODO(alice):`, or `None` when the marker is unowned.
    pub owner: Option<String>,
    /// The text following the marker, trimmed. May be empty.
    pub note: String,
    /// 1-based line number within the scanned text.
    pub line: usize,
}

impl Marker {
    /// An unowned marker is one written without an `(owner)` segment.
    pub fn is_unowned(&self) -> bool {
        self.owner.is_none()
    }
}

/// True when `b` cannot be part of an identifier, i.e. it is a word boundary.
fn is_boundary(b: u8) -> bool {
    !(b.is_ascii_alphanumeric() || b == b'_')
}

/// Find the first marker in a single line, if any.
///
/// Recognises `TODO`, `TODO:`, `TODO(owner)` and `TODO(owner): note`. The
/// keyword must stand alone as a word, so `TODOS` and `MY_TODO` do not match.
pub fn find_marker_in_line(
    line: &str,
    kinds: &[MarkerKind],
) -> Option<(MarkerKind, Option<String>, String)> {
    let bytes = line.as_bytes();
    let mut best: Option<(usize, MarkerKind)> = None;

    for &kind in kinds {
        let kw = kind.keyword();
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(kw) {
            let start = from + rel;
            let end = start + kw.len();
            let left_ok = start == 0 || is_boundary(bytes[start - 1]);
            let right_ok = end == bytes.len() || is_boundary(bytes[end]);
            if left_ok && right_ok {
                if best.is_none_or(|(b, _)| start < b) {
                    best = Some((start, kind));
                }
                break;
            }
            from = end;
        }
    }

    let (start, kind) = best?;
    let rest = &line[start + kind.keyword().len()..];
    let (owner, note) = split_owner_and_note(rest);
    Some((kind, owner, note))
}

/// Split the text following a marker keyword into `(owner, note)`.
fn split_owner_and_note(rest: &str) -> (Option<String>, String) {
    let trimmed = rest.trim_start();
    if let Some(after_paren) = trimmed.strip_prefix('(') {
        if let Some(close) = after_paren.find(')') {
            let owner = after_paren[..close].trim().to_string();
            let note = after_paren[close + 1..]
                .trim_start()
                .trim_start_matches(':')
                .trim()
                .to_string();
            // `TODO():` names nobody, which still counts as unowned.
            let owner = if owner.is_empty() { None } else { Some(owner) };
            return (owner, note);
        }
    }
    (None, trimmed.trim_start_matches(':').trim().to_string())
}

/// Scan a block of text and return every marker it contains.
pub fn scan_text(text: &str, kinds: &[MarkerKind]) -> Vec<Marker> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            find_marker_in_line(line, kinds).map(|(kind, owner, note)| Marker {
                kind,
                owner,
                note,
                line: i + 1,
            })
        })
        .collect()
}

// ── Workspace scanning ─────────────────────────────────────────────────

/// A marker together with the file it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMarker {
    /// Path relative to the scan root, using forward slashes.
    pub file: String,
    #[serde(flatten)]
    pub marker: Marker,
}

/// Directory names never descended into during a workspace scan.
///
/// Deliberately excludes `bin`: `src/bin/` is ordinary Rust source, and skipping
/// it would silently under-report. Compiled artifacts are already filtered out
/// by [`SCAN_EXTENSIONS`], so naming `bin` here would cost real coverage and buy
/// nothing.
const SKIP_DIRS: [&str; 5] = ["target", ".git", "node_modules", "dist", ".venv"];

/// File extensions considered scannable source text.
const SCAN_EXTENSIONS: [&str; 14] = [
    "rs", "toml", "md", "js", "ts", "tsx", "jsx", "py", "go", "sh", "json", "yml", "yaml", "txt",
];

fn is_scannable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SCAN_EXTENSIONS.contains(&e))
}

/// Recursively collect scannable files beneath `root`, sorted for determinism.
pub fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) || name.starts_with('.') {
                continue;
            }
            walk(&path, out);
        } else if kind.is_file() && is_scannable(&path) {
            out.push(path);
        }
    }
}

/// Normalise a path relative to `root` into a forward-slash string.
fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Scan every scannable file beneath `root` for markers.
///
/// Files that cannot be read as UTF-8 are skipped rather than failing the scan,
/// and the result is capped at `max_results`.
pub fn scan_workspace(root: &Path, kinds: &[MarkerKind], max_results: usize) -> Vec<FileMarker> {
    let mut out = Vec::new();
    for path in collect_files(root) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for marker in scan_text(&text, kinds) {
            if out.len() >= max_results {
                return out;
            }
            out.push(FileMarker {
                file: relative_display(root, &path),
                marker,
            });
        }
    }
    out
}

// ── Settings ───────────────────────────────────────────────────────────

/// Plugin configuration, read from `.claude/claude-example.local.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// When true, the hook reports markers written without an owner.
    pub require_owner: bool,
    /// Which marker kinds to scan for.
    pub kinds: Vec<MarkerKind>,
    /// Upper bound on markers returned by a workspace scan.
    pub max_results: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            require_owner: true,
            kinds: MarkerKind::ALL.to_vec(),
            max_results: 200,
        }
    }
}

impl Config {
    /// Parse settings from the YAML frontmatter of a `.local.md` file.
    ///
    /// Unknown keys and malformed values are ignored: a broken settings file
    /// degrades to the defaults rather than breaking the user's session.
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
                "require_owner" => match value {
                    "true" => config.require_owner = true,
                    "false" => config.require_owner = false,
                    _ => {}
                },
                "kinds" => {
                    let parsed: Vec<MarkerKind> = value
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .split(',')
                        .filter_map(MarkerKind::from_keyword)
                        .collect();
                    if !parsed.is_empty() {
                        config.kinds = parsed;
                    }
                }
                "max_results" => {
                    if let Ok(n) = value.parse::<usize>() {
                        if n > 0 {
                            config.max_results = n;
                        }
                    }
                }
                _ => {}
            }
        }
        config
    }

    /// Load settings for `project_dir`, falling back to defaults when the file
    /// is absent or unreadable.
    pub fn load(project_dir: &Path) -> Self {
        let path = project_dir.join(".claude").join("claude-example.local.md");
        match std::fs::read_to_string(path) {
            Ok(text) => Config::from_markdown(&text),
            Err(_) => Config::default(),
        }
    }
}

/// Return the text between the leading `---` fences, if present.
fn extract_frontmatter(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("---")?.trim_start_matches(['\r', '\n']);
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[MarkerKind] = &MarkerKind::ALL;

    // ── marker parsing ─────────────────────────────────────────────

    #[test]
    fn finds_owned_todo() {
        let (kind, owner, note) =
            find_marker_in_line("// TODO(alice): wire up retries", ALL).expect("marker");
        assert_eq!(kind, MarkerKind::Todo);
        assert_eq!(owner.as_deref(), Some("alice"));
        assert_eq!(note, "wire up retries");
    }

    #[test]
    fn finds_unowned_todo() {
        let (kind, owner, note) = find_marker_in_line("// TODO: wire up retries", ALL).unwrap();
        assert_eq!(kind, MarkerKind::Todo);
        assert_eq!(owner, None);
        assert_eq!(note, "wire up retries");
    }

    #[test]
    fn bare_keyword_has_empty_note() {
        let (kind, owner, note) = find_marker_in_line("// TODO", ALL).unwrap();
        assert_eq!(kind, MarkerKind::Todo);
        assert_eq!(owner, None);
        assert_eq!(note, "");
    }

    #[test]
    fn empty_owner_parens_count_as_unowned() {
        let (_, owner, note) = find_marker_in_line("// TODO(): something", ALL).unwrap();
        assert_eq!(owner, None);
        assert_eq!(note, "something");
    }

    #[test]
    fn keyword_must_stand_alone() {
        assert!(find_marker_in_line("let TODOS = 3;", ALL).is_none());
        assert!(find_marker_in_line("let MY_TODO = 3;", ALL).is_none());
        assert!(find_marker_in_line("fn todo_list() {}", ALL).is_none());
    }

    #[test]
    fn recognises_fixme_and_hack() {
        let (kind, ..) = find_marker_in_line("# FIXME(bob): broken", ALL).unwrap();
        assert_eq!(kind, MarkerKind::Fixme);
        let (kind, ..) = find_marker_in_line("// HACK: temporary", ALL).unwrap();
        assert_eq!(kind, MarkerKind::Hack);
    }

    #[test]
    fn returns_earliest_marker_on_a_line() {
        // FIXME appears before TODO, so it wins regardless of scan order.
        let (kind, ..) = find_marker_in_line("// FIXME: a  // TODO: b", ALL).unwrap();
        assert_eq!(kind, MarkerKind::Fixme);
    }

    #[test]
    fn respects_the_configured_kind_subset() {
        let only_todo = [MarkerKind::Todo];
        assert!(find_marker_in_line("// FIXME: nope", &only_todo).is_none());
        assert!(find_marker_in_line("// TODO: yes", &only_todo).is_some());
    }

    // ── text scanning ──────────────────────────────────────────────

    #[test]
    fn scan_text_reports_one_based_line_numbers() {
        let src = "fn main() {}\n// TODO(alice): first\n\n// FIXME: second\n";
        let markers = scan_text(src, ALL);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].line, 2);
        assert_eq!(markers[0].owner.as_deref(), Some("alice"));
        assert_eq!(markers[1].line, 4);
        assert!(markers[1].is_unowned());
    }

    #[test]
    fn scan_text_on_clean_source_is_empty() {
        assert!(scan_text("fn main() {}\nlet x = 1;\n", ALL).is_empty());
    }

    // ── settings ───────────────────────────────────────────────────

    #[test]
    fn missing_frontmatter_yields_defaults() {
        assert_eq!(Config::from_markdown("# just prose\n"), Config::default());
    }

    #[test]
    fn parses_every_setting() {
        let src =
            "---\nrequire_owner: false\nkinds: [TODO, FIXME]\nmax_results: 25\n---\n\nprose\n";
        let config = Config::from_markdown(src);
        assert!(!config.require_owner);
        assert_eq!(config.kinds, vec![MarkerKind::Todo, MarkerKind::Fixme]);
        assert_eq!(config.max_results, 25);
    }

    #[test]
    fn malformed_values_fall_back_to_defaults() {
        let src = "---\nrequire_owner: maybe\nkinds: [NOPE]\nmax_results: 0\n---\n";
        assert_eq!(Config::from_markdown(src), Config::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let src = "---\nrequire_owner: false\nnonsense: 42\n---\n";
        let config = Config::from_markdown(src);
        assert!(!config.require_owner);
        assert_eq!(config.max_results, Config::default().max_results);
    }

    #[test]
    fn config_load_without_a_settings_file_is_default() {
        let dir = std::env::temp_dir().join("claude-example-no-settings");
        assert_eq!(Config::load(&dir), Config::default());
    }

    // ── serialization contract ─────────────────────────────────────

    #[test]
    fn file_marker_serializes_flat_with_uppercase_kind() {
        let fm = FileMarker {
            file: "src/lib.rs".into(),
            marker: Marker {
                kind: MarkerKind::Todo,
                owner: Some("alice".into()),
                note: "ship it".into(),
                line: 7,
            },
        };
        let json = serde_json::to_value(&fm).unwrap();
        assert_eq!(json["file"], "src/lib.rs");
        assert_eq!(json["kind"], "TODO");
        assert_eq!(json["owner"], "alice");
        assert_eq!(json["line"], 7);
    }
}
