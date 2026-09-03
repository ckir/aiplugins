## What version of Qwen Code are you using?

0.22.3

## What operating system are you using?

Windows 11 (win32)

## Describe the bug

Running `qwen extensions install` with a direct `.zip` archive URL exits with code 0, produces zero output on stdout or stderr, and does not install the extension. `qwen extensions list` still shows nothing.

## Steps to reproduce

1. Have any valid extension bundle zip hosted at a public URL (must contain `qwen-extension.json` at the root)
2. Run: `qwen extensions install <archive-url> --consent`
3. Run: `qwen extensions list`

**Expected**: Extension is downloaded, extracted, and appears in the list.

**Actual**: Command exits 0 with completely empty output. No extension is installed.

## Exact reproduction

**Command**: `qwen extensions install https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip --consent`

**Output**: (completely empty — no stdout, no stderr)

**Exit code**: `0`

**After**: `qwen extensions list` outputs `No extensions installed.`

Same result with `-d` (debug) flag — still zero output.

## Environment

- OS: Windows 11 (win32)
- Terminal: Interactive Windows Terminal, cmd.exe, real TTY (stdin not redirected/piped)
- `qwen --version`: `0.22.3`
- `--consent` flag is explicitly passed (no interactive prompt)
- CLI installed via: official installer / npm global

## Workaround that works

Manually downloading and extracting the same zip to `C:\Users\user\.qwen\extensions\rtk-mcp-qwen\` works — the extension is correctly detected and enabled by `qwen extensions list`.

Installing from a git repo URL (`qwen extensions install https://github.com/...`) also works (shows interactive marketplace selector).

The silent exit-0 only happens with direct `.zip` archive URLs.

## Test archive

Public zip for testing: https://github.com/ckir/aiplugins/releases/download/v0.6.4/rtk-mcp-qwen-extension.zip

Contents: `qwen-extension.json`, `bin/` (cross-platform binaries + dispatcher), `QWEN.md`, `README.md`, `LICENSE`.
