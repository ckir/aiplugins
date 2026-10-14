# aiplugins-devtools-opencode

An OpenCode plugin that ships dev-tool skills. The first skill is
`code-review-and-quality`, a multi-axis code review (correctness, readability,
architecture, security, performance) with security and performance checklists.
It carries only skills: no plugin code, no hooks, no MCP servers, no binaries.

## Installation

See [INSTALL.md](INSTALL.md). It is a file copy of the skill folder into
OpenCode's `skills/` directory; there is no `opencode.jsonc` to merge.

## Invocation

OpenCode keys a skill by its folder name, so here the skill is invoked as
`aiplugins-devtools-code-review-and-quality`. A colon is not valid in a folder
name, so the `aiplugins-devtools:` form used on the other agents is not
available on OpenCode.

## Attribution

The skill is adapted from Addy Osmani's agent-skills; see
`skills/aiplugins-devtools-code-review-and-quality/THIRD-PARTY-NOTICE.md` for
attribution and the upstream licence. The canonical source lives in
`shared/devtools/` and is copied here, with the folder and frontmatter name
prefixed, by `just emit-devtools-skills`.
