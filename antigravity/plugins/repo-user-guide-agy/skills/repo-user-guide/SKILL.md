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
- A composite example (e.g. add, list, done in sequence) is fine if every step was really run; say "assembled from real runs".
- Long output (more than about 25 lines): never cut it mid-line or mark a gap with "...". A single long line is not "long output"; keep it whole and let it wrap. Show it in full; or keep whole lines and add a note under the block ("output trimmed to the relevant lines"); or, for very long output (telemetry dumps, verbose logs), summarize it in prose and say it is a summary. Anything you shorten, the reader must be able to tell.
- Output that differs on every run (timestamps, PIDs, ports, durations, random ids, ANSI colour codes): paste one real run and say once, under the block, what varies ("timestamps and ports differ per run"). If you strip colour codes or normalise a value, say so rather than doing it silently. Don't invent a "typical" value you never saw.
- An environment variable you exported for the whole session (for instance `NO_COLOR=1` to avoid colour codes in captured output) changes what a reader sees. Say so once near the top ("outputs were captured with TASKER_NO_COLOR=1"), and show the real default where it matters, for example one example run without it.
- A CLI that is not on PATH: show commands the way a user will type them once installed (`tasker add ...`), and say once near the top how you actually ran them (`node bin/tasker.js`, or a shim on PATH). Do not pretend the install step was run.
- Never include real secrets; use placeholders like `YOUR_API_KEY`.

### 5. Write HTML, render to PDF
Copy `assets/guide-template.html` and fill it in. Write plain HTML (no Markdown converter is assumed installed). The template gives you: `<pre><code>` for commands/code, `<pre class="output">` for results, `.note`/`.warn` callouts, anchored `<h2 id>` sections with a linked table of contents, and a cover page. Do these in order: **write, then escape, then check, then render.**

**Writing code and output.** Classes combine, in any order: `raw` marks text for escaping, `output` styles a result, `wide` keeps columns aligned (help text, tables) by not re-wrapping lines (about 110 characters fit, at a small 7.5pt font, so use it only where alignment matters; a long single-line output such as one-line JSON stays non-wide so it wraps instead of being clipped). Write `<pre class="raw"><code>…</code></pre>` for commands and `<pre class="output raw">` for results, with the text pasted exactly as it appears in the file or terminal. `raw` also works on inline code in prose: `<code class="raw">` plus the literal text (a tag name like `<title>`, say), otherwise the browser swallows it as markup. Never write `&lt;` yourself, and if you generate the HTML from a script, do not `html.escape` the text first, or it is escaped twice. Content that is already escaped (a reused draft, say) must not be marked `raw`.

**Commands and their output.**
- Wrap examples in `<div class="ex">…</div>`: the template's `.ex` rule keeps everything inside it on one page, so a page break never leaves a command at the bottom of one page and its output on the next. Size the wrapper to the example: a short example (several commands and outputs that fit in about 30 lines, roughly a third of a page) gets ONE wrapper around the whole sequence, so a break cannot fall between its commands. A longer example gets one wrapper per command-and-output pair. "Short" is a judgement: when in doubt, ask whether the whole example would look wrong split across two pages. A series of short examples that build on each other under separate headings (list, then filter, then clean up) is separate examples: one wrapper each, with a one-line note saying it continues the previous state. Do not wrap every command separately by default: many small unbreakable blocks leave gaps and add pages. A command with no output (an illustrative one) goes inside the wrapper of the example it belongs to, or gets its own.
- Prefix each command line with `$ ` (one per command, also in multi-line blocks) so commands and their output are never confused. Show an env var inline (`$ TASKER_FILE=notes.json tasker list`). A `# comment` line inside a command block is fine. A file's contents are not a command: show them as `$ cat file` followed by an output block, and make sure the file really has that content when you run it.
- Show an exit-code check as its own command (`$ echo $?`) whose output is the number, and capture the real value: a script that runs each command in a fresh shell, or prints its own marker between commands, resets `$?` to 0, so record the status right after the command and check the numbers in the rendered PDF; do not append the number to another command's output block, which should hold only what the tool printed.
- When a command writes to both stdout and stderr and the order matters (an error followed by usage text, say), run it with `2>&1` and show the combined output in the order the terminal printed it, with the `2>&1` visible in the command.
- Examples that build on each other (ids, files created by earlier steps) must either run in sequence in one scratch directory and appear in the same order, or, if you ran one against fresh state, carry a short note under it ("run in a fresh directory, so the id is 1") so a reader following along is not confused by numbers that differ.
- Tag every example heading with a badge: `<h3>Add a task <span class="tag ran">ran</span></h3>` for an example you executed, `<span class="tag ill">illustrative</span>` for one you did not. A composite stays `ran` with a sentence under it ("assembled from real runs"). An example with only some commands run is split into two examples, one `ran` and one `illustrative`, so a badge never overstates what was verified. Count these badges for the final report.

**Escape, then check.**
```bash
python3 <skill-dir>/scripts/escape_pre.py guide.html guide.html   # escapes every raw element, drops the marker
python3 <skill-dir>/scripts/check_pre.py guide.html                # lint; fix every finding, then re-run
```
`escape_pre.py` touches only `<pre>` and `<code>` elements whose class list contains the token `raw`, and is safe to re-run. Run it BEFORE `check_pre.py`: on an unescaped draft the check just reports one "still marked raw" finding per block. `check_pre.py` flags `wide` lines over 110 characters, a command block followed by an output block that is not wrapped in `<div class="ex">`, blocks or inline code still marked `raw`, and text that shows an entity such as `&lt;` after decoding (a double escape).


Render with:
```bash
bash <skill-dir>/scripts/html_to_pdf.sh docs/USER_GUIDE.html docs/USER_GUIDE.pdf
```
It uses headless Chrome/Chromium, prints the page count, and warns when Chrome reported load errors or the PDF has almost no text (Chrome still writes a PDF when part of a page fails, so a non-empty file is not proof of a good render). If no Chrome/Chromium is present, tell the user and leave the HTML.

### 6. Check the result
Open the PDF with the Read tool (`pages` needs poppler's `pdftoppm`) or run `pdftoppm -r 60 -png`; `pdftotext` and `pdfinfo` give text and page count. If poppler is missing, screenshot the HTML with headless Chrome (`--screenshot`) instead. Look for: cut-off code lines, a command separated from its output across a page break (`pdftotext -f N -l N` on each page: a page ending in a `$ ` line while the next starts with `Output`), orphaned headings, empty sections, broken TOC links, leftover `{{PLACEHOLDER}}`, or an `…` placeholder (deliberate `...` in real code like spreads is fine). Fix and re-render.

### 7. Report
Give the path, page count, how many examples were actually executed vs. marked illustrative (count one example = one titled block, however many commands it holds), and any discrepancies found in the repo. Don't commit unless asked.

## Quality bar
- Every feature in the public surface is covered or deliberately excluded.
- Examples are copy-pasteable and use realistic values, not `foo`/`bar` when a real use case is easy.
- Prose is short; explain the *why* only where a user would otherwise get stuck.
- Version and date on the cover come from the repo (manifest/tags), not guesswork.
