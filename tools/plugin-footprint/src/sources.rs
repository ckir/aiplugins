//! Reading a plugin's file-backed sources (spec §4.5).

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
    /// A file name that is not valid UTF-8.
    ///
    /// Refused rather than converted. `to_string_lossy` maps EVERY invalid byte
    /// sequence onto U+FFFD, so two different names arrive as one id; two
    /// sources then share an id and their relative order comes from `read_dir`
    /// rather than from the data, churning the committed document between
    /// machines. Making the sort key `(kind, id)` was specifically to stop that,
    /// and a lossy id defeats it. `document::strip_root` refuses to compare
    /// lossily for the same reason.
    #[error("{path} has a name that is not valid UTF-8; rename it")]
    NotUtf8 { path: PathBuf },
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
        expected: "skills/<name>/SKILL.md",
        resident_kind: "skill_frontmatter",
        invocation_kind: "skill_body",
    },
    Layout {
        dir: "agents",
        nested: false,
        expected: "agents/<name>.md",
        resident_kind: "agent_frontmatter",
        invocation_kind: "agent_body",
    },
    Layout {
        dir: "commands",
        nested: false,
        expected: "commands/<name>.md",
        resident_kind: "command_frontmatter",
        invocation_kind: "command_body",
    },
];

struct Layout {
    dir: &'static str,
    /// `true` for `skills/<id>/SKILL.md`, `false` for `<dir>/<id>.md`.
    nested: bool,
    /// The shape a source in this directory must have, quoted back in the error
    /// so a contributor is sent to the right file. Every `Malformed` used to say
    /// `skills/<name>/SKILL.md` whatever the directory, which was harmless only
    /// while the nested layout was the only one that could produce one.
    expected: &'static str,
    resident_kind: &'static str,
    invocation_kind: &'static str,
}

/// Files a human's tools leave behind in any directory they have opened.
///
/// Skipped rather than reported, and this is the ONE silent skip this module
/// tolerates — an OS artifact is neither a source nor a mistake about a source.
///
/// MEASURED during the capstone review: a `.DS_Store`, which appears the moment
/// a macOS developer opens `skills/` in Finder, took the whole measurement down
/// with "skills/.DS_Store is not where a source belongs". Under `set -e` that
/// aborts `footprint-regen`, so the CI job fails with an error about a skill
/// that does not exist. Being loud is right for a source in the wrong shape; it
/// is hostile for a file that was never a source.
fn is_os_artifact(name: &str) -> bool {
    name.starts_with('.')
        || name.eq_ignore_ascii_case("Thumbs.db")
        || name.eq_ignore_ascii_case("desktop.ini")
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
            let raw = std::fs::read_to_string(&path).map_err(|source| SourceError::Read {
                path: path.clone(),
                source,
            })?;
            // Line endings normalised before ANYTHING is counted.
            //
            // Git stores these files with LF and hands a Windows checkout CRLF,
            // so without this the same commit measures differently on a Windows
            // developer's machine and on Linux CI — MEASURED at one byte per
            // line, 139 of them in `agents/re-analyst.md` alone. §6's freshness
            // layer is "regenerate, then require no diff", so a platform-
            // dependent count fails it on every run for a reason that has
            // nothing to do with a footprint. Removing the document's timestamp
            // was meant to make that layer meaningful; this is the other half of
            // the same guarantee.
            //
            // Counting the CONTENT rather than the checkout is also the more
            // honest measurement. A user installs a bundle assembled by CI, so
            // LF is what they actually pay for; the CRLF is an artefact of one
            // developer's working copy and nobody's context window.
            let text = if raw.contains('\r') {
                raw.replace("\r\n", "\n")
            } else {
                raw
            };

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
    out.resident
        .sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    out.invocation
        .sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
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
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(SourceError::NotUtf8 { path: entry.path() });
        };

        // Only FILES. Skipping every dot-named entry skipped dot-named
        // DIRECTORIES too, so `skills/.sneaky/SKILL.md` would have vanished from
        // the measurement with nothing failing — a way to hide cost, created by
        // the fix for a way to fail loudly. A dot-named directory goes through
        // the normal check instead: it is either a source or malformed, and both
        // are loud.
        if !entry.path().is_dir() && is_os_artifact(&name) {
            continue;
        }

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
                    expected: layout.expected,
                });
            }
            found.push((name, file));
        } else if let Some(id) = name.strip_suffix(".md") {
            found.push((id.to_string(), entry.path()));
        } else {
            // The flat layouts were the asymmetry. A skill in the wrong shape
            // was already loud; an agent in the wrong shape was silently
            // dropped, because anything not ending `.md` simply fell out of the
            // loop. MEASURED: `agents/re-analyst.md.bak` and a nested
            // `agents/reviewer/agent.md` both vanished from the measurement with
            // nothing failing anywhere — the false zero this crate refuses,
            // arriving through the one directory nobody thought about.
            return Err(SourceError::Malformed {
                path: entry.path(),
                expected: layout.expected,
            });
        }
    }
    Ok(found)
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
