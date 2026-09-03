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
    let resolved = lexically_normalize(&anchored);
    let root = lexically_normalize(plugin_dir);

    let escapes = match (&resolved, &root) {
        (Some(resolved), Some(root)) => !resolved.starts_with(root),
        // A path that climbs above its own prefix is not inside anything.
        _ => true,
    };
    if escapes {
        return Err(ManifestError::CommandEscapesPluginRoot {
            server: name.to_string(),
            command,
            plugin_dir: plugin_dir.to_path_buf(),
        });
    }
    let resolved = resolved.expect("checked above");

    Ok(ServerSpec {
        name: name.to_string(),
        command: resolved,
        args: raw.args,
        env: raw.env,
    })
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
