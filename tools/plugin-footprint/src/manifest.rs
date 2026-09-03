//! Reading a plugin's `.mcp.json` and turning it into something launchable (spec §4.2).
//!
//! The manifest already carries everything needed to start a server — `command`,
//! `args`, `env` — so a new plugin is measurable with no change to this tool.
//! That is the whole reason the launch is manifest-driven rather than
//! special-cased per plugin.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// The placeholder Claude Code expands to the installed plugin's directory.
///
/// Qwen writes `${extensionPath}` instead, and separates path segments with a
/// `${/}` token rather than a literal `/` — commit 306c7be in this repo is a fix
/// for mis-parsing exactly that. Qwen support is deferred (spec §8, Fork 3), and
/// when it lands it belongs in a sibling reader, not in this constant.
const PLUGIN_ROOT: &str = "${CLAUDE_PLUGIN_ROOT}";

/// One MCP server, resolved against a plugin directory and ready to launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    /// The key under `mcpServers`, used to name this server's sources in the
    /// footprint document.
    pub name: String,
    /// ABSOLUTE, always. A relative command whose normalised form loses its
    /// separator — `./cmd.exe` becoming `cmd.exe` — is not the same instruction
    /// to the OS: `Command::new` resolves a bare name through `PATH`
    /// (CreateProcessW on Windows, execvp on Unix), so confinement would
    /// authorise a path inside the plugin while something else entirely got
    /// launched. Storing the absolute form removes that whole class.
    pub command: PathBuf,
    pub args: Vec<String>,
    /// Only what the manifest declares. The prober adds a minimal platform
    /// allowlist at launch and passes nothing else through (spec §4.2).
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("server '{server}' in {path} declares no command")]
    MissingCommand { server: String, path: PathBuf },
    #[error(
        "server '{server}' declares a command outside its plugin directory: {command} resolves \
         outside {plugin_dir}. The prober launches this command, so it is confined to the plugin \
         it belongs to."
    )]
    CommandEscapesPluginRoot {
        server: String,
        command: String,
        plugin_dir: PathBuf,
    },
}

#[derive(Deserialize)]
struct Manifest {
    /// Absent in a plugin that ships hooks or skills but no MCP server.
    #[serde(rename = "mcpServers", default)]
    mcp_servers: BTreeMap<String, RawServer>,
}

#[derive(Deserialize)]
struct RawServer {
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

/// Whether `dir` is a plugin directory at all.
///
/// A plugin with no `.mcp.json` is legitimate — it may ship only hooks or skills
/// — so "no servers" cannot distinguish a hooks-only plugin from a mistyped
/// path. This can. Without it, pointing the tool at the wrong directory produces
/// a confident `status: ok, bytes: 0`, which is the same false zero the probe
/// status exists to prevent, arriving one level up.
pub fn looks_like_a_plugin(dir: &Path) -> bool {
    dir.join(".claude-plugin").join("plugin.json").is_file()
}

/// Read `<plugin_dir>/.mcp.json` and resolve every server it declares.
///
/// A plugin with no `.mcp.json` declares no servers, which is not an error — a
/// hooks-only or skills-only plugin simply has no MCP footprint. A manifest that
/// exists but cannot be parsed IS an error: reporting "no servers" for a file we
/// failed to read would understate the footprint and quietly pass the gate.
///
/// Servers come back sorted by name so the footprint document does not inherit
/// the order the JSON happened to be written in.
pub fn read_mcp_servers(plugin_dir: &Path) -> Result<Vec<ServerSpec>, ManifestError> {
    let path = plugin_dir.join(".mcp.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(ManifestError::Read { path, source }),
    };

    // `serde_json` is configured for camelCase at the field, not the container,
    // because `mcpServers` is the only key this tool reads and spelling it here
    // keeps the struct honest about what the file actually contains.
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|source| ManifestError::Parse {
            path: path.clone(),
            source,
        })?;

    manifest
        .mcp_servers
        .into_iter()
        .map(|(name, raw)| resolve(&name, raw, plugin_dir, &path))
        .collect()
}

fn resolve(
    name: &str,
    raw: RawServer,
    plugin_dir: &Path,
    manifest_path: &Path,
) -> Result<ServerSpec, ManifestError> {
    let command = raw.command.ok_or_else(|| ManifestError::MissingCommand {
        server: name.to_string(),
        path: manifest_path.to_path_buf(),
    })?;

    // Substituting the placeholder ALREADY anchors the path to the plugin
    // directory, so joining afterwards would prepend it a second time. That is
    // invisible while the plugin directory is absolute — the substituted path is
    // then absolute too and `Path::join` replaces rather than appends — and
    // wrong the moment it is relative, which is the normal case:
    // `plugin-footprint measure claude-code/re-ghidra-mcp-cc`.
    //
    // A command with no placeholder is interpreted relative to the plugin, which
    // is what the join is for.
    let anchored = if command.contains(PLUGIN_ROOT) {
        PathBuf::from(command.replace(PLUGIN_ROOT, &plugin_dir.to_string_lossy()))
    } else {
        plugin_dir.join(&command)
    };

    // Normalising also settles separators: substitution can leave a Windows root
    // spliced onto forward slashes (`C:\plugin/bin/x`), and `PathBuf` compares
    // as raw text, so an un-normalised path would differ from the same location
    // written natively.
    // The absolute form is what gets stored and launched, so that normalising
    // can never change what executing the path MEANS. The document records a
    // plugin-relative path separately (`document::build`).
    let resolved = absolutize(&anchored);

    if !is_confined(plugin_dir, &anchored) {
        return Err(ManifestError::CommandEscapesPluginRoot {
            server: name.to_string(),
            command,
            plugin_dir: plugin_dir.to_path_buf(),
        });
    }
    let resolved = resolved.expect("confinement rejects a path that cannot be absolutised");

    Ok(ServerSpec {
        name: name.to_string(),
        command: resolved,
        args: raw.args,
        env: raw.env,
    })
}

/// Whether `command` stays inside `plugin_dir`.
///
/// Both sides are made absolute first, and that is the whole point. `Path`'s
/// `starts_with` returns TRUE for an empty prefix — every path starts with
/// nothing — and a plugin directory of `.`, `./` or `""` normalises to exactly
/// that, since `lexically_normalize` drops `CurDir` components. Comparing
/// against the bare normalised root would therefore confine nothing at all for
/// `plugin-footprint measure .`, which is an entirely ordinary invocation.
/// MEASURED: with a root of `.`, `C:/Windows/System32/evil.exe` was accepted.
///
/// The absolute forms are used ONLY for this decision. What gets launched, and
/// what the document records, stays as written — the document's binary path is
/// repo-relative by design (spec §5) and must not become machine-specific.
fn is_confined(plugin_dir: &Path, command: &Path) -> bool {
    let (Some(root), Some(candidate)) = (absolutize(plugin_dir), absolutize(command)) else {
        // Either side unresolvable, or a path that climbed above its own prefix:
        // fail closed.
        return false;
    };
    // Defence in depth: a root with no components confines nothing, and must
    // never be treated as a root even if `absolutize` were to yield one.
    if root.components().next().is_none() {
        return false;
    }
    candidate.starts_with(&root)
}

/// A lexically normalised absolute form, resolving a relative path against the
/// process working directory.
///
/// Public because the document needs the same notion of "absolute" to compute a
/// plugin-relative path from an absolute command.
///
/// Filesystem-free, like `lexically_normalize`: the command routinely does not
/// exist yet when this runs.
pub fn absolutize(path: &Path) -> Option<PathBuf> {
    let anchored = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    lexically_normalize(&anchored)
}

/// Resolve `.` and `..` textually, without consulting the filesystem.
///
/// Lexically, not via `canonicalize`: a plugin's `bin/` is gitignored and built
/// on demand, so the command routinely does not exist yet when this runs, and
/// `canonicalize` fails on a path that is not there. Resolving `..` textually is
/// what lets the confinement check work before the build.
///
/// The trade-off is that a symlink inside the plugin pointing out of it would
/// pass. Nothing in this repository ships one, and the alternative is a check
/// that cannot run at all in the case it exists for.
///
/// Returns `None` if the path climbs above its own root, which cannot be inside
/// anything and is always a refusal.
fn lexically_normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // `pop` returns false when there is nothing left to remove, i.e.
                // the path has escaped above its own prefix.
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(command: &str) -> RawServer {
        RawServer {
            command: Some(command.to_string()),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    fn resolve_for(plugin_dir: &str, command: &str) -> Result<ServerSpec, ManifestError> {
        let dir = PathBuf::from(plugin_dir);
        resolve("s", raw(command), &dir, &dir.join(".mcp.json"))
    }

    /// These exercise `resolve` directly rather than through a fixture on disk,
    /// so they can use a RELATIVE plugin directory without any test having to
    /// `set_current_dir` — which is process-global and races the moment the
    /// harness runs tests on threads.
    #[test]
    fn a_directory_that_is_not_a_plugin_is_recognisable() {
        // The repo root is not a plugin; a plugin directory is.
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crate sits two levels below the repo root");

        assert!(!looks_like_a_plugin(repo));
        assert!(looks_like_a_plugin(
            &repo.join("claude-code").join("re-ghidra-mcp-cc")
        ));
    }

    #[test]
    fn a_relative_plugin_directory_appears_once_not_twice() {
        // Found by running the tool, not by the suite: substituting the
        // placeholder already anchors the path, so joining afterwards prepended
        // the plugin directory a second time. Invisible with an absolute plugin
        // directory, because the substitution is then absolute too and
        // `Path::join` replaces rather than appends.
        let spec = resolve_for("claude-code/x", "${CLAUDE_PLUGIN_ROOT}/bin/s-mcp")
            .expect("a relative plugin directory resolves");

        // Absolute (see `ServerSpec::command`), and the plugin directory appears
        // exactly once within it.
        let expected = absolutize(Path::new("claude-code/x"))
            .expect("absolutises")
            .join("bin")
            .join("s-mcp");
        assert_eq!(spec.command, expected);
    }

    #[test]
    fn a_command_without_the_placeholder_is_taken_relative_to_the_plugin() {
        let spec = resolve_for("claude-code/x", "bin/s-mcp").expect("resolves");

        let expected = absolutize(Path::new("claude-code/x"))
            .expect("absolutises")
            .join("bin")
            .join("s-mcp");
        assert_eq!(spec.command, expected);
    }

    #[test]
    fn a_plugin_directory_of_dot_still_confines() {
        // `Path::starts_with` returns true for an empty prefix, and `.`
        // normalises to empty — so comparing against the bare normalised root
        // confined nothing at all. `measure .` is an ordinary invocation, which
        // is what made this reachable.
        for root in [".", "./", ""] {
            let err = resolve_for(root, "C:/Windows/System32/evil.exe")
                .expect_err("an absolute outside command must be refused");
            assert!(
                matches!(err, ManifestError::CommandEscapesPluginRoot { .. }),
                "root {root:?} gave {err}"
            );
            let err = resolve_for(root, "/etc/passwd")
                .expect_err("a unix absolute path must be refused too");
            assert!(matches!(
                err,
                ManifestError::CommandEscapesPluginRoot { .. }
            ));
        }
    }

    #[test]
    fn a_bare_command_name_never_stays_bare() {
        // MEASURED: `lexically_normalize` drops the CurDir component, so a
        // command of "cmd.exe" under a plugin directory of "." came out as the
        // bare string "cmd.exe". `Command::new` treats a name with no separator
        // as a PATH lookup (CreateProcessW on Windows, execvp on Unix), so
        // confinement authorised `./cmd.exe` inside the plugin while the OS
        // launched C:\Windows\System32\cmd.exe. Normalising a path changed
        // what executing it MEANS.
        for root in [".", "./", "plug"] {
            let spec = resolve_for(root, "cmd.exe").expect("resolves");
            assert!(
                spec.command.is_absolute(),
                "root {root:?} produced {:?}, which the OS may resolve through PATH",
                spec.command
            );
        }
    }

    #[test]
    fn a_dot_root_still_accepts_something_genuinely_inside_it() {
        // The fix must not confine by refusing everything.
        let spec = resolve_for(".", "${CLAUDE_PLUGIN_ROOT}/bin/s-mcp").expect("resolves");

        assert!(spec.command.ends_with(Path::new("bin").join("s-mcp")));
    }

    #[test]
    fn every_occurrence_of_the_placeholder_is_substituted() {
        // `str::replace` replaces all occurrences; this pins that a second one
        // cannot survive into a launched command as a literal `${...}`.
        let spec = resolve_for("plug", "${CLAUDE_PLUGIN_ROOT}/bin/${CLAUDE_PLUGIN_ROOT}x")
            .expect("resolves");

        assert!(
            !spec.command.to_string_lossy().contains("${"),
            "an unsubstituted placeholder would be launched literally: {:?}",
            spec.command
        );
    }
}
