---
name: repo-user-guide
description: Create a user guide, as a PDF, for a code repository, with many practical, verified usage examples. Use whenever the user asks for a user guide, usage guide, manual, handbook, "how to use this repo/library/CLI", getting-started documentation, or a PDF of documentation for a repo or package, even if they don't say "PDF" or "user guide" explicitly. Not for API-reference generation, README-only edits, or internal architecture docs.
---

# Repo User Guide (PDF)

Produce a PDF that teaches a *user* (not a contributor) how to use a repository. The value is in examples: a reader should be able to copy a snippet and have it work. Examples that were never run are the main way these guides go wrong, so run them.

## Workflow

### 1. Establish scope (briefly)
Target repo = the given path, else the current working directory. Identify the audience (library consumer, CLI user, API/service operator) from the repo itself; only ask the user if it's genuinely ambiguous. Output goes to `<repo>/docs/USER_GUIDE.pdf` unless a path is given or the repo must stay untouched (keep the HTML source next to it).

### 2. Learn the repo from primary sources
Read, in this order, stopping when you understand the public surface: README, AGENTS.md/CLAUDE.md (agent context often lists commands), package manifest (`package.json` `bin`/`exports`/`scripts`, `pyproject.toml`, `Cargo.toml`, etc.), entry points, existing `docs/`, `examples/`, tests (tests are the best source of correct usage), CLI `--help` output, and config/env var definitions. For big repos, delegate wide read-only sweeps to a cheap subagent that returns a summary with file paths; keep the writing yourself.

Build a list of the **public surface**: install steps, CLI commands and flags, exported functions/classes, config keys, env vars, file formats. Document **only public APIs**. Anything not exported from a package entry point, not in `bin`, or not documented as user-facing stays out, even if it is easy to call by deep import. Use the package's public import specifiers in examples, never internal file paths. If the docs disagree with the code on a public API, report it.

### 3. Plan the outline
Adapt to the repo; a typical shape:
1. Overview (what it is, who it's for, 1 minimal example)
2. Installation & requirements
3. Quick start (shortest path to a working result)
4. Core concepts (only those needed to understand examples)
5. Usage by task or feature, the bulk of the guide, each with examples
6. Configuration reference (table: key, type, default, meaning)
7. Recipes / common workflows (multi-step, realistic)
8. Troubleshooting (real errors you saw or that the code raises, with fixes)
9. Appendix: command/API quick reference

Order by what users do, not by how the source tree is laid out.

### 4. Write examples, then verify them
Give each feature **3-5 examples** (fewer if the feature only has that many natural cases; never pad): the simplest case, a common variation or two, and an error or edge case where one matters. More than 5 is padding. Show the command/code *and* its output.

Verification matters because a guide with wrong flags destroys trust. For every example:
- Run it when it is safe and local, in whatever way you judge best (scratch dir, copy of the repo, symlinked packages, local stub servers). Never write into the repo itself unless asked. Paste the *actual* output.
- If it can't be run safely (needs credentials, network, destructive, paid services, deploys), do not run it; derive it from tests/source and mark the output `(illustrative)` or omit the output. Never invent output.
- If you find the code or docs disagree with behavior, report it to the user instead of silently picking one.
- A composite example (e.g. add, list, done in sequence) is fine if every step was really run; say "assembled from real runs". Never elide output with "..."; show it in full or trim to whole lines and say so.
- Never include real secrets; use placeholders like `YOUR_API_KEY`.

### 5. Write HTML, render to PDF
Copy `assets/guide-template.html` and fill it in. Write plain HTML (no Markdown converter is assumed installed). Conventions in the template: `<pre><code>` for commands/code, `<pre class="output">` for results, `.note`/`.warn` callouts, anchored `<h2 id>` sections with a linked table of contents, cover page. For code, write `<pre class="raw"><code>…</code></pre>` with the code pasted as-is, then run `python3 <skill-dir>/scripts/escape_pre.py guide.html guide.html` to escape `<`, `>`, `&` (hand-escaping is error-prone). Tag each example heading as `ran` or `illustrative`.

Render with:
```bash
bash <skill-dir>/scripts/html_to_pdf.sh docs/USER_GUIDE.html docs/USER_GUIDE.pdf
```
It uses headless Chrome/Chromium. If none is present, tell the user and leave the HTML.

### 6. Check the result
Open the PDF with the Read tool (`pages` needs poppler's `pdftoppm`) or run `pdftoppm -r 60 -png`; `pdftotext` and `pdfinfo` give text and page count. If poppler is missing, screenshot the HTML with headless Chrome (`--screenshot`) instead. Look for: cut-off code lines, orphaned headings, empty sections, broken TOC links, leftover `{{PLACEHOLDER}}`, or an `…` placeholder (deliberate `...` in real code like spreads is fine). Fix and re-render.

### 7. Report
Give the path, page count, how many examples were actually executed vs. marked illustrative (count one example = one titled block), and any discrepancies found in the repo. Don't commit unless asked.

## Quality bar
- Every feature in the public surface is covered or deliberately excluded.
- Examples are copy-pasteable and use realistic values, not `foo`/`bar` when a real use case is easy.
- Prose is short; explain the *why* only where a user would otherwise get stuck.
- Version and date on the cover come from the repo (manifest/tags), not guesswork.
