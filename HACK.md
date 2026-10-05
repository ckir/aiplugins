# HACK.md — contributor notes

## Adding or removing a supported agent

The set of supported agents lives in one place: `agents.json` at the repo
root. Every wiring and marketplace check (`scripts/check-marketplace.sh`,
`scripts/check-qwen-marketplace.sh`, `scripts/check-plugin-wiring.sh`,
`scripts/check-opencode-wiring.sh`, via `scripts/lib/registry.sh`) enumerates
agents from that registry, so a new agent needs no script edits — register it
and every check picks it up.

### To add an agent

1. Copy the template front directory. `claude-code/example/` is the template
   Rust plugin front end; duplicate the agent directory closest to the new
   host's format and rename it to the new agent's `dir`.
2. Add one entry to `agents.json` with all eight fields:
   `id`, `dir`, `plugins`, `marketplace` (a path like
   `.claude-plugin/marketplace.json`, or `null` when the host has no
   marketplace manifest), `manifestPattern` (with `{plugin}` for the per-plugin
   manifest path), `artifactSuffix`, `sourceUrlBase`, and `notPublished`
   (plugin stems the checks skip, e.g. `example`).
3. If the agent has a marketplace (`marketplace` is not `null`), regenerate
   the host manifests: `bash scripts/gen-marketplaces.sh`.
4. Run the Task 6 gate command — `bash scripts/gen-marketplaces.sh --check`
   (the same step CI runs, plus the `just gen-marketplaces` recipe) — and the
   wiring checks (`just marketplace qwen-marketplace wiring opencode-wiring`).
   Every check picks the new agent up with no script edits.

### To remove an agent

1. Delete the agent's directory.
2. Drop its entry from `agents.json`.
3. Regenerate (`bash scripts/gen-marketplaces.sh`) and re-run the gate
   command from step 4 above.

Support is best-effort by design: an agent nobody registers in `agents.json`
is invisible to all gates — nothing fails, nothing checks it.

## Footprint gate override (stale-base corrections only)

The delta cap compares fresh measurements against the merge-base document.
If the base document itself is stale, the correction PR trips the cap with
zero new source bytes. Procedure: verify with
`git diff origin/main...HEAD -- <measured dirs>` (must be empty of measured
inputs), admin-merge with `footprint-override: <reason>` in the merge
message. Each use is greppable via `git log --grep=footprint-override`.

## Tags and releases

Never push tags by hand — per-package `*-v*` tags are created only by
release-please after its bump PR merges, and tag protection enforces it.
In particular never run `git push --tags`: it pushes every local tag,
including stale or experimental ones, and each matching tag mints a real
release. Footprint gate overrides use `footprint-override: <reason>` in the
merge message (see the `footprint` recipe notes in the Justfile).

## Marketplace URLs are pinned per release (regen after every bump)

Marketplace `source.url` entries are tag-pinned
(`releases/download/<package>-v<version>/<file>`), rendered from each
package's own manifest by `bash scripts/gen-marketplaces.sh`. There is no
`latest` anywhere: one repo-wide `latest` cannot serve eight packages.

After every release-please bump PR, the committed marketplaces are stale by
construction (manifests moved, urls did not), and CI fails the bump PR on
`gen-marketplaces.sh --check` plus the host wiring checks. Procedure: check
out the release-please branch (`release-please--branches--main`), run
`bash scripts/gen-marketplaces.sh`, commit, push. Merge the bump PR only
once that commit is green. The new marketplace points at the about-to-exist
release; dist + bundle workflows attach the zips within minutes of the tag.

## Tag pushes that must trigger workflows (read before hand-pushing tags)

Two GitHub behaviors have bitten this repo; both are silent (no error, no
run, nothing anywhere to say why):

1. Tags created by release-please never trigger tag-push workflows.
   release-please pushes with the default `GITHUB_TOKEN`, and GitHub does
   not start workflow runs for `GITHUB_TOKEN`-created pushes. Concretely:
   after every release PR merges, its tags exist and its GitHub releases
   exist (empty), but dist never builds and no bundle workflow ever fires.
   Until release-please runs under a PAT (open issue), backfill by hand:
   delete each empty release (`gh release delete <tag> --yes`), delete each
   remote tag, recreate the identical tag at the identical commit, and push.
2. Never push more than ~3 tags for one commit in a single push. Past that,
   GitHub assumes a mistake and silently starts zero runs. Push tags one at
   a time (`git push origin <tag>`, confirm the run appears, repeat).
