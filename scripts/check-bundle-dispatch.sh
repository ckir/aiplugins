#!/usr/bin/env bash
#
# Drive every branch of the plugin bin dispatcher.
#
# scripts/plugin-bin-dispatch.sh.in is shipped inside every plugin bundle as
# `bin/<name>`, and it is the one file there that no compiler sees and no test
# suite runs. Worse, any single machine exercises exactly one of its branches:
# 0.6.0 shipped a dispatcher that rejected MINGW outright, which was invisible
# from Linux, invisible from a direct spawn on Windows (that resolves the .exe
# without ever reading this file), and fatal to a Git Bash hook.
#
# So the platform is faked. A stub `uname` on PATH answers for each target in
# turn, and stand-in scripts replace the binaries, which makes the assertion
# simply "which file did it exec".
set -euo pipefail

cd "$(dirname "$0")/.."

template=scripts/plugin-bin-dispatch.sh.in
[ -f "$template" ] || {
    echo "ERROR: $template not found." >&2
    exit 1
}

name=probe
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/stub"

sed "s/__NAME__/$name/g" "$template" > "$work/bin/$name"
chmod 755 "$work/bin/$name"

cat > "$work/stub/uname" << 'STUB'
#!/bin/sh
case $1 in
    -m) echo "$FAKE_ARCH" ;;
    *) echo "$FAKE_OS" ;;
esac
STUB
chmod 755 "$work/stub/uname"

printf '#!/bin/sh\necho windows-exe\n' > "$work/bin/$name.exe"
chmod 755 "$work/bin/$name.exe"

# The unix targets the bundler lays out, taken from the same place it takes
# them, so a new target in dist-workspace.toml shows up here as a failure
# rather than as an untested branch.
targets=$(sed -n 's/^targets = \[\(.*\)\]$/\1/p' dist-workspace.toml |
    tr ',' '\n' | tr -d ' "' | grep -v '^$' | grep -v windows)
[ -n "$targets" ] || {
    echo "ERROR: no unix targets parsed from dist-workspace.toml." >&2
    exit 1
}
for target in $targets; do
    mkdir -p "$work/bin/$target"
    printf '#!/bin/sh\necho %s\n' "$target" > "$work/bin/$target/$name"
    chmod 755 "$work/bin/$target/$name"
done

failures=0
checked=0

# check <uname -s> <uname -m> <expected output> [expected exit status]
check() {
    checked=$((checked + 1))
    local expect_status=${4:-0} got status
    got=$(FAKE_OS="$1" FAKE_ARCH="$2" PATH="$work/stub:$PATH" "$work/bin/$name" 2>&1) &&
        status=0 || status=$?
    if [ "$got" != "$3" ] || [ "$status" != "$expect_status" ]; then
        printf '  FAIL  uname -s %-24s -m %-8s gave %-48s (exit %s), wanted %s (exit %s)\n' \
            "$1" "$2" "'$got'" "$status" "'$3'" "$expect_status" >&2
        failures=$((failures + 1))
    else
        printf '  ok    uname -s %-24s -m %-8s -> %s\n' "$1" "$2" "$got"
    fi
}

check Linux x86_64 x86_64-unknown-linux-gnu
check Linux aarch64 aarch64-unknown-linux-gnu
check Darwin x86_64 x86_64-apple-darwin
check Darwin arm64 aarch64-apple-darwin

# Every shell that reports itself as Windows has to reach the .exe. These are
# the three uname flavours a bundled hook can actually meet there.
check MINGW64_NT-10.0-26200 x86_64 windows-exe
check MSYS_NT-10.0-26200 x86_64 windows-exe
check CYGWIN_NT-10.0 x86_64 windows-exe

check SunOS x86_64 "$name: unsupported operating system SunOS" 1
check Linux mips "$name: unsupported architecture mips" 1

# A missing binary must say so rather than exec whatever is nearby.
rm -f "$work/bin/x86_64-unknown-linux-gnu/$name"
check Linux x86_64 "$name: this plugin ships no binary for x86_64-unknown-linux-gnu" 1

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: the dispatcher routes $failures of $checked case(s) wrong." >&2
    exit 1
fi

echo "Dispatcher routes all $checked case(s) correctly."
