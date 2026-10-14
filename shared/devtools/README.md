# shared/devtools

Maintainer note for the `aiplugins-devtools` skills. It sits outside `skills/`
on purpose, so the drift check ignores it.

## Layout

- `shared/devtools/skills/<skill>/` is the single source of the skills.
- `just emit-devtools-skills` copies it into the four plugins
  (`claude-code/plugins/aiplugins-devtools-cc`, `qwen/plugins/aiplugins-devtools-qwen`,
  `antigravity/plugins/aiplugins-devtools-agy`, `opencode/plugins/aiplugins-devtools-opencode`).
  OpenCode gets the prefixed folder name (`aiplugins-devtools-<skill>`) and one
  rewritten `name:` line in its `SKILL.md`.
- Checks: `just devtools-skills` (source and copies), `just skills-bundles`
  (the bundled zips) and `just skills-release-wiring` (the release workflows).
- `agents.json` marks these plugins with `publishedNames` (folder name to the
  published name `aiplugins-devtools`) and `skillsOnly` (no crate, no binary).

## Adding a second skill

Nothing discovers skills automatically. Edit by hand:

- the `emit-devtools-skills` recipe in the `Justfile`, which hardcodes
  `code-review-and-quality` for the OpenCode copy (prefixed folder plus the
  `name:` rewrite);
- the copies list in `scripts/check-devtools-skills.sh`;
- note that `scripts/check-skills-bundles.sh` needs no edit: its `pairs` list
  names the four host plugins, and it finds skills with `find`;
- note that the drift check only compares the named skill folder, so a new skill
  is not covered until it is added to that list.

## First release order

The four packages are seeded at `0.1.0` in `.release-please-manifest.json`. The
marketplace entries on `main` point at
`.../aiplugins-devtools-<x>-v0.1.0/...` URLs that do not exist until the tags
are created, so until then `claude plugin install aiplugins-devtools@aiplugins`
fails (404).

Create the four tags by hand BEFORE merging any release-please PR. Push at most
three tags at a time: GitHub drops tag-push events when more than three tags
land on one commit (see the comment in `.github/workflows/release.yml`). For
example:

```text
git tag aiplugins-devtools-cc-v0.1.0 <commit>
git push origin aiplugins-devtools-cc-v0.1.0
```

`.github/workflows/skills-bundles.yml` then builds and uploads each zip and
creates the GitHub Release if it is missing. How release-please treats these
seeded packages afterwards has NOT been tested.

## What has been exercised

Validated with `claude plugin validate --strict`, by loading the bundle zip with
`claude --plugin-dir` (init lists `aiplugins-devtools:code-review-and-quality`),
and with `agy plugin validate`.

NOT exercised: OpenCode, Qwen Code, GitHub Actions, release-please.
