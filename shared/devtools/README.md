# shared/devtools

Maintainer note for the `aiplugins-devtools` skills. It sits outside `skills/`
on purpose, so the drift check ignores it.

## Layout

- `shared/devtools/skills/<skill>/` is the single source of the skills.
- `shared/devtools/commands/<name>.md` is the single source of the OpenCode
  slash-command files. Claude Code, Qwen Code and Antigravity make every skill a
  `/` command by themselves; OpenCode lists no skill in its `/` menu, so it needs
  a command file that loads the skill (its prefixed name) and takes the target
  path as `$ARGUMENTS`.
- `just emit-devtools-skills` copies them into the four plugins
  (`claude-code/plugins/aiplugins-devtools-cc`, `qwen/plugins/aiplugins-devtools-qwen`,
  `antigravity/plugins/aiplugins-devtools-agy`, `opencode/plugins/aiplugins-devtools-opencode`).
  OpenCode gets the prefixed folder name (`aiplugins-devtools-<skill>`) and one
  rewritten `name:` line in its `SKILL.md`, plus the command files in `commands/`.
- Checks: `just devtools-skills` (source and copies), `just skills-bundles`
  (the bundled zips) and `just skills-release-wiring` (the release workflows).
- `agents.json` marks these plugins with `publishedNames` (folder name to the
  published name `aiplugins-devtools`) and `skillsOnly` (no crate, no binary).

## Adding another skill

Skills are discovered from `shared/devtools/skills/*/`, so the emit recipe and
the drift check (`scripts/check-devtools-skills.sh`) pick a new folder up by
themselves. Do this:

- create `shared/devtools/skills/<skill>/SKILL.md` with `name: <skill>` as its
  frontmatter name (the drift check fails otherwise);
- if the skill should be a slash command on OpenCode, add
  `shared/devtools/commands/<skill>.md`: a `description:` frontmatter and a body
  that names `aiplugins-devtools-<skill>` and contains `$ARGUMENTS` (the drift
  check requires both);
- run `just emit-devtools-skills`, then `just devtools-skills` and
  `just skills-bundles`;
- run `just footprint-regen`: the resident cost of every new `SKILL.md`
  frontmatter (and every OpenCode command file's frontmatter) counts against the
  500-byte per-change delta cap in `docs/footprints/budgets.json`, and new growth
  that trips it is never overridden, so keep frontmatter short.

Only a vendored upstream skill needs a `THIRD-PARTY-NOTICE.md`; the bundler asks
for one somewhere in the zip, which `code-review-and-quality` supplies.

## First release order

The four packages are seeded at `0.1.0` in `.release-please-manifest.json`. The
marketplace entries on `main` point at
`.../aiplugins-devtools-<x>-v0.1.0/...` URLs that do not exist until the tags
are created, so until then `claude plugin install aiplugins-devtools@aiplugins`
fails (404).

Create the four tags by hand BEFORE the change that seeds them merges to
`main`, or immediately after it. The Release Please workflow runs on every push
to `main` and looks up a release for each seeded package; until
`aiplugins-devtools-<x>-v0.1.0` exists it fails ("Missing 4 paths" in its log).
This was measured on the first merge. Push at most three tags at a time: GitHub
drops tag-push events when more than three tags land on one commit (see the
comment in `.github/workflows/release.yml`). For example:

```text
git tag -a aiplugins-devtools-cc-v0.1.0 <commit>
git push origin aiplugins-devtools-cc-v0.1.0
```

`.github/workflows/skills-bundles.yml` then builds and uploads each zip and
creates the GitHub Release if it is missing (exercised on the first release: all
four runs succeeded and each Release carries its zip). With the tags in place
release-please finds all its releases; a later failure while it walks the merge
commits ended with an empty error message and is not yet explained.

## What has been exercised

Validated with `claude plugin validate --strict`, by loading the bundle zip with
`claude --plugin-dir` (init lists `aiplugins-devtools:code-review-and-quality`),
and with `agy plugin validate`.

NOT exercised: OpenCode and Qwen Code themselves, and release-please creating a
release PR for these packages. The `skills-bundles.yml` workflow has run for all
four tags.
