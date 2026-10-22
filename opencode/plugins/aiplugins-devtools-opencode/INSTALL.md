# Installing aiplugins-devtools-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw; do not improvise workarounds
> that change what gets installed.
>
> Everything below comes from a versioned release asset: no branch files, no
> unpinned URLs. Agree the version with the user first (`0.1.0` is current as
> of this writing; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use it for every URL below.

You will install `aiplugins-devtools-opencode`: a pure skills plugin that
ships two skills (a multi-axis code review and an adversarial spec or plan
review) and one slash-command file. Install is a file copy of the skill folders
and of that command file. There is no `opencode.jsonc` to merge, no binary to
stage and no dependency to install.

This document is the executable form of `README.md`. If the two ever
disagree, this file wins for install mechanics and the mismatch is a bug;
tell the user.

## Step 0 - prerequisites

Confirm each one; stop and report if one fails:

1. OpenCode V2 is installed: `opencode --version` prints a V2 version.
2. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).

## Step 1 - ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` accordingly and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 - fetch the bundle and copy the skills and the command

Set the agreed version once:

```bash
VERSION=0.1.0
```

Download the versioned bundle: asset `aiplugins-devtools-opencode-opencode.zip`
on release `aiplugins-devtools-opencode-v$VERSION` (the doubled name is
`<package>-opencode.zip` for package `aiplugins-devtools-opencode`). Stage it
outside `$DEST`:

```bash
curl -fsSL -o /tmp/aiplugins-devtools-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/aiplugins-devtools-opencode-v$VERSION/aiplugins-devtools-opencode-opencode.zip"
rm -rf /tmp/aiplugins-devtools-opencode-stage && mkdir -p /tmp/aiplugins-devtools-opencode-stage
unzip -q -o /tmp/aiplugins-devtools-opencode.zip -d /tmp/aiplugins-devtools-opencode-stage
```

The zip's top level IS the plugin directory (no wrapper folder). Copy each
whole skill folder, including `references/` and `THIRD-PARTY-NOTICE.md` where
present, into `$DEST/skills/`, and the command file into `$DEST/command/`
(OpenCode does not list skills in its `/` menu, so the slash command is a
command file):

```bash
mkdir -p "$DEST/skills" "$DEST/command"
for skill in aiplugins-devtools-code-review-and-quality aiplugins-devtools-spec-panel-review; do
  rm -rf "$DEST/skills/$skill"
  cp -r "/tmp/aiplugins-devtools-opencode-stage/skills/$skill" "$DEST/skills/"
done
cp /tmp/aiplugins-devtools-opencode-stage/commands/spec-panel-review.md "$DEST/command/"
```

Do not rename the folders: OpenCode requires a skill's folder name to equal
the `name:` in its `SKILL.md` frontmatter, and each pair is
`aiplugins-devtools-<skill>`. Every download and copy must exit 0.

## Step 3 - verify the install

1. `$DEST/skills/aiplugins-devtools-code-review-and-quality/SKILL.md` exists
   and is non-empty, and the `references/` folder and
   `THIRD-PARTY-NOTICE.md` sit beside it.
2. `$DEST/skills/aiplugins-devtools-spec-panel-review/SKILL.md` exists and is
   non-empty, and `$DEST/command/spec-panel-review.md` exists.
3. The first lines of each `SKILL.md` contain `name:` followed by its folder
   name (`aiplugins-devtools-code-review-and-quality`,
   `aiplugins-devtools-spec-panel-review`).
4. Restart OpenCode (skills and commands load at startup), then check that both
   skills appear in OpenCode's skill list and that `/spec-panel-review` appears
   in its `/` menu. If either does not appear, report that to the user rather
   than editing OpenCode's configuration.

This install procedure has not been exercised against a running OpenCode by
the authors of this document; step 4 is the first real test, so report
exactly what you see.

## Invoking the skill

Invoke a skill by its prefixed folder name,
`aiplugins-devtools-code-review-and-quality` or
`aiplugins-devtools-spec-panel-review`, or run `/spec-panel-review <path>` to
panel-review a spec or plan. A colon is not valid in a folder
name, so the `aiplugins-devtools:` form used on other agents is not available
on OpenCode.
