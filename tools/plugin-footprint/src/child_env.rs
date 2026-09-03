//! The environment a probed server is launched with (spec §4.2).
//!
//! Allowlisted, not inherited. The published figure is regenerated in a
//! release-time job because that is where `ANTHROPIC_API_KEY` lives, and §4.1
//! has that same job launch plugin binaries — so an inherited environment would
//! hand the key to every server the prober starts, during a handshake the server
//! itself drives. Nothing about measuring a tool schema needs the parent's
//! secrets.
//!
//! Allowlisted rather than empty, though. A process with no `PATH` cannot
//! resolve anything it shells out to, and on Windows one without `SystemRoot`
//! fails before `main` in most runtimes; an empty environment would make the
//! prober report `failed` for every plugin, and §6's fatal probe assertion would
//! turn that into a permanently red build. Securing the measurement by breaking
//! it is not securing it.

use std::collections::BTreeMap;

/// Variables a child needs to start at all, on any platform we build for.
///
/// Deliberately short. Anything a plugin genuinely needs beyond this it declares
/// in its own manifest `env`, where the requirement is visible in review.
const PLATFORM_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "TMPDIR",
    "TEMP",
    "TMP",
    "LANG",
    "LC_ALL",
    // Windows. A process without SystemRoot fails to initialise its C runtime
    // and networking stack long before it reaches our handshake.
    "SystemRoot",
    "SystemDrive",
    "WINDIR",
];

/// Build the environment for a probed server: the platform allowlist filtered
/// out of `inherited`, plus everything the manifest `declared`.
///
/// `declared` wins on a collision. The manifest is the plugin's own statement of
/// what it needs, and an ambient value of the same name must not silently
/// override it.
///
/// `inherited` is passed in rather than read from the process so that callers —
/// tests especially — never have to mutate global environment state, which races
/// under a parallel test runner.
pub fn child_env(
    declared: &BTreeMap<String, String>,
    inherited: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = inherited
        .iter()
        .filter(|(name, _)| is_allowlisted(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();

    for (name, value) in declared {
        // On Windows the environment BLOCK is case-insensitive but this map is
        // not, so `PATH` and `Path` both survive as separate keys — and
        // `Command::envs` applies them in map order, letting the alphabetically
        // later spelling win. MEASURED: a map of {"FOO": declared, "foo":
        // ambient} produced a child that saw the AMBIENT value, exactly
        // reversing the rule this function documents. Dropping every other
        // spelling first is what makes "declared wins" true rather than stated.
        if cfg!(windows) {
            out.retain(|existing, _| !existing.eq_ignore_ascii_case(name));
        }
        out.insert(name.clone(), value.clone());
    }

    out
}

/// The current process environment, in the shape `child_env` wants.
pub fn inherited_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

fn is_allowlisted(name: &str) -> bool {
    // Windows environment blocks are case-insensitive, and a runner may expose
    // `SYSTEMROOT` where another exposes `SystemRoot`. Matching case-sensitively
    // there would strip the variable the allowlist exists to keep.
    if cfg!(windows) {
        PLATFORM_ALLOWLIST
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(name))
    } else {
        PLATFORM_ALLOWLIST.contains(&name)
    }
}
