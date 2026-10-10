#!/usr/bin/env bash
#
# Exercise each opencode/plugins/ plugin's setup path without an OpenCode install.
#
# plugin.ts imports the real @opencode/plugin runtime (installed per plugin
# via `bun install`, as in the opencode-test CI job), so bun can import the
# real module and drive setup() against a fake ctx. The rtk hook then runs
# its REAL spawn path against the compiled mock-rtk-cc fixture copied onto
# PATH as `rtk` — the same fixture the Rust e2e suite drives.
#
# Coverage limit: this proves the setup + spawn path only, NOT OpenCode's own
# event plumbing (that half belongs to the Task 1 shell-shape spike).
set -euo pipefail

cd "$(dirname "$0")/.."

failures=0
fail() { echo "  FAIL  $1" >&2; failures=$((failures + 1)); }

# Cargo exposes the fixture as CARGO_BIN_EXE_mock-rtk-cc (hyphens intact),
# which POSIX shells can neither `export` nor expand as $VAR. Pass it with
#   env "CARGO_BIN_EXE_mock-rtk-cc=$(pwd)/target/debug/mock-rtk-cc" bash scripts/smoke-opencode.sh
# (append `.exe` on Windows) and read it back with printenv, which takes the
# name as an argument and never parses it as an identifier.
mock_bin=$(printenv 'CARGO_BIN_EXE_mock-rtk-cc' || true)
[ -x "$mock_bin" ] || {
    echo "ERROR: mock-rtk-cc not built. Run: cargo build -p rtk-mcp-cc --bin mock-rtk-cc, then re-run via: env \"CARGO_BIN_EXE_mock-rtk-cc=\$(pwd)/target/debug/mock-rtk-cc\" bash scripts/smoke-opencode.sh" >&2
    exit 1
}
command -v bun > /dev/null || { echo "ERROR: bun is not on PATH." >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# Windows resolves `rtk` on PATH to rtk.exe (PATHEXT), never to an
# extensionless file — so stage the fixture under the platform's name. An
# extensionless copy is invisible on Windows: the spawn then falls through to
# any ambient real rtk on PATH, or fails open with no rewrite at all.
exe=""
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN* | Windows_NT) exe=".exe" ;;
esac
cp "$mock_bin" "$work/rtk$exe"
chmod +x "$work/rtk$exe"
export PATH="$work:$PATH"

for dir in opencode/plugins/*/; do
    name=$(basename "$dir")
    echo "== $name"

    bun -e "
import plugin from './$dir/plugin.ts';
const hooks = {};
const ctx = {
  tool: { hook: async (n, f) => { (hooks['tool:' + n] ??= []).push(f); } },
  session: { hook: async (n, f) => { (hooks['session:' + n] ??= []).push(f); } },
  storage: { get: async () => undefined, set: async () => {} },
};
await plugin.setup(ctx);
const names = Object.keys(hooks).sort().join(',');
if (!names) throw new Error('no hooks registered');
console.log('  hooks: ' + names);
" || { fail "$name: setup() threw under bun"; continue; }

    if [ "$name" = "rtk-mcp-opencode" ]; then
        bun -e "
import plugin from './$dir/plugin.ts';
let seen = '';
const ctx = {
  tool: { hook: async (n, f) => {
    if (n !== 'execute.before') return;
    const event = { tool: 'shell', input: { command: 'cat README.md' } };
    f(event);
    seen = event.input.command;
  } },
  session: { hook: async () => {} },
  storage: { get: async () => undefined, set: async () => {} },
};
process.env.RTK_BIN = 'rtk';
await plugin.setup(ctx);
// The mock fixture echoes the command verbatim as 'rtk <command>'; require
// exactly that. A looser 'non-empty' check false-passes when the mock is
// missed — a dead RTK_BIN (fail-open, command untouched) or an ambient real
// rtk on PATH (a semantic rewrite like 'rtk read README.md') both slip
// through. With this assertion a dead RTK_BIN throws by design.
if (seen !== 'rtk cat README.md') throw new Error('expected the mock rewrite, got: ' + JSON.stringify(seen));
console.log('  rewrite: ' + seen);
" || fail "$name: end-to-end rewrite through mock rtk failed"
    fi
done

if [ "$failures" -gt 0 ]; then
    echo >&2; echo "ERROR: $failures opencode smoke problem(s)." >&2; exit 1
fi
echo "Opencode smoke passed for all plugins."
