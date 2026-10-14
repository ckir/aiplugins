# aiplugins-devtools-agy

An Antigravity plugin that ships dev-tool skills. The first skill is
`code-review-and-quality`, a multi-axis code review (correctness, readability,
architecture, security, performance) with security and performance checklists.
It carries only skills: no hooks, no MCP servers, no binaries.

## Installation

1. **Download the plugin bundle** for tag `aiplugins-devtools-agy-v0.1.0`. The
   asset name is `<package>-agy-plugin.zip` for package
   `aiplugins-devtools-agy`, hence the doubled `agy`:

   ```bash
   curl -fsSL -o /tmp/aiplugins-devtools-agy-agy-plugin.zip \
     https://github.com/ckir/aiplugins/releases/download/aiplugins-devtools-agy-v0.1.0/aiplugins-devtools-agy-agy-plugin.zip
   unzip -q -o /tmp/aiplugins-devtools-agy-agy-plugin.zip -d /tmp/aiplugins-devtools
   ```

   Extract into a directory named `aiplugins-devtools` (the manifest `name`),
   not after the repository folder: Antigravity keys installed plugins by
   directory name, so this is the name you will see in your configuration.

2. **Install the plugin** from the extracted directory:

   ```bash
   agy plugin install /tmp/aiplugins-devtools
   ```

   This copies the directory into place; after downloading a newer bundle,
   install again from its fresh extraction.

## Attribution

The skill is adapted from Addy Osmani's agent-skills; see
`skills/code-review-and-quality/THIRD-PARTY-NOTICE.md` for attribution and the
upstream licence. The canonical source lives in `shared/devtools/` and is copied
here by `just emit-devtools-skills`.
