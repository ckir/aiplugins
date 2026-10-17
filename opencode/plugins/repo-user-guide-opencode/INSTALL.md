# Installing repo-user-guide-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw; do not improvise workarounds.
>
> Everything below comes from a versioned release asset: no branch files, no
> unpinned URLs. Agree the version with the user first (`0.2.0` is current as
> of this writing; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use it for every URL below.

You will install `repo-user-guide-opencode`: a pure skills plugin that ships
the `repo-user-guide` skill (PDF user guides with verified examples). The
install is a file copy of one folder. There is no `opencode.jsonc` to merge,
no binary to stage and no dependency to install.

This document is the executable form of `README.md`. If the two ever disagree,
this file wins for install mechanics and the mismatch is a bug; tell the user.

## Step 0 - prerequisites

1. OpenCode V2 is installed: `opencode --version` prints a V2 version.
2. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).
3. The skill's own runtime requirements matter at use time, not install time:
   Chrome/Chromium, Python 3, and (recommended) poppler.

## Step 1 - ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 - fetch the bundle and copy the skill

Set the agreed version once:

```bash
VERSION=0.2.0
```

Download the versioned bundle: asset `repo-user-guide-opencode-opencode.zip`
on release `repo-user-guide-opencode-v$VERSION` (the doubled name is
`<package>-opencode.zip` for package `repo-user-guide-opencode`). Stage it
outside `$DEST`:

```bash
curl -fsSL -o /tmp/repo-user-guide-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/repo-user-guide-opencode-v$VERSION/repo-user-guide-opencode-opencode.zip"
rm -rf /tmp/repo-user-guide-opencode-stage && mkdir -p /tmp/repo-user-guide-opencode-stage
unzip -q -o /tmp/repo-user-guide-opencode.zip -d /tmp/repo-user-guide-opencode-stage
```

Copy the skill folder:

```bash
rm -rf "$DEST/skills/repo-user-guide"
mkdir -p "$DEST/skills"
cp -r "/tmp/repo-user-guide-opencode-stage/skills/repo-user-guide" \
      "$DEST/skills/repo-user-guide"
```

## Step 3 - verify

Confirm each; stop and report if one fails:

```bash
test -s "$DEST/skills/repo-user-guide/SKILL.md" && echo OK skill
test -s "$DEST/skills/repo-user-guide/assets/guide-template.html" && echo OK template
test -s "$DEST/skills/repo-user-guide/scripts/html_to_pdf.sh" && echo OK html_to_pdf
test -s "$DEST/skills/repo-user-guide/scripts/check_pre.py" && echo OK check_pre
test -s "$DEST/skills/repo-user-guide/scripts/escape_pre.py" && echo OK escape_pre
```

Also confirm the `frontmatter name:` in
`$DEST/skills/repo-user-guide/SKILL.md` is `repo-user-guide` — OpenCode keys
skills by folder name, so folder and name must match.

## Step 4 - clean up

```bash
rm -rf /tmp/repo-user-guide-opencode.zip /tmp/repo-user-guide-opencode-stage
```

## Usage

OpenCode auto-discovers the skill; ask for "a user guide for this repo" (or
invoke `repo-user-guide` directly). The skill writes the HTML and PDF next to
the repository. Nothing is registered in any `opencode.jsonc`.
