---
name: aiplugins-devtools-human-friendly
description: Rewrite a Markdown file in place so it reads naturally, keeping code, links, tables, numbers and meaning intact. Use to humanize, de-robotize or polish a README or doc. Supports --review-only and --dry-run.
---

# Human-Friendly

Edit a Markdown document the way an experienced technical editor would: clearer, more natural, same meaning. This is a writing-quality skill. It does not detect AI text, and it must never promise to evade detectors.

## Input

The first argument that is not a flag is the target path; with no path and no clear target in context, ask one short question. Several paths are processed one at a time, each reported separately. Never sweep a directory when one file was named.

| Option | Effect |
|---|---|
| (none) | Review, rewrite and write the file |
| `--review-only` | Report findings only; write nothing |
| `--dry-run` | Prepare and validate the rewrite, show a summary and a diff if you can; write nothing |
| `--profile <name>` | Pick a style: `natural-professional` (default), `conversational`, `executive` |

An unknown option or profile, or a conflicting pair such as `--review-only --dry-run`, gets an explanation, not a guess. The same goes for plain requests like "make this easier to read".

| Profile | Style | Suits |
|---|---|---|
| `natural-professional` | Clear, direct, approachable; keeps technical terms | READMEs, specs, architecture docs |
| `conversational` | Relaxed and personal, never slangy or forced | Tutorials, blog posts, community docs |
| `executive` | Concise, decision-oriented: outcomes, risks, actions; keeps every caveat | Proposals, reports |

## Writing principles

- Prefer familiar words, direct statements, short and varied sentences, active voice where it helps, one idea per paragraph.
- Explain unavoidable jargon for the likely audience. Prefer precise statements over vague ones.
- Fix: robotic or convoluted phrasing, filler, empty openers and generic conclusions, repetition, inconsistent terms, unsupported marketing language, stacked adjectives, needless passive voice.
- Cut formulaic phrases that carry no meaning ("In today's rapidly evolving landscape", "It is important to note that", mechanical "Furthermore", "Let's dive into", "Unlock the power of", unearned "seamlessly" or "robust and scalable"). This is guidance, not a banned list: keep a phrase that is right.
- Do not over-edit. Leave wording that is already clear. A document that needs no change stays unchanged, and saying so is a good result. Do not shorten at the cost of detail, turn technical prose into marketing, make it casual by force, or add flaws to look human.

## Preserve exactly

These outrank style. Keep them byte for byte unless the user says otherwise:

- YAML frontmatter, whole block. If it is malformed or its boundaries are unclear, leave it and report it.
- Fenced, indented and structured blocks (code, Mermaid, JSON, YAML, TOML, XML, math, embedded HTML or scripts): contents, indentation and line endings.
- Inline code, commands, flags, identifiers, config keys and values, file paths.
- URLs, link destinations, reference-link definitions, image destinations and alt text.
- Table data, row and column counts and order, alignment markers, and any links or code inside cells.
- Direct quotations and their attribution, citations, license and legal text.
- Versions, numbers, units, dates, thresholds.
- Requirements, constraints, assumptions, acceptance criteria, security warnings.

Never translate, rename or "correct" a protected identifier.

You may edit prose in headings (same meaning and level), paragraphs, list items, table-cell prose, blockquote prose that is not a quotation, and captions. Link text may change only if its meaning stays equivalent.

Never change the strength or scope of a claim. Keep must/should/may, always/sometimes/never, supported/planned/experimental, required/optional, possible/guaranteed, and current behavior versus future intent. "Supports A and B" must not become "supports all major providers"; "may fail" must not become "recovers automatically"; "may support recovery" must not become a promise of recovery. Invent no feature, guarantee, performance claim, compatibility or evidence. If the original is ambiguous, keep the ambiguity and report it.

## Workflow

1. **Resolve the target.** Resolve relative paths from the project root. It must exist, be a regular file, and end in `.md` or `.markdown`. Do not leave the intended project, follow a symlink out of it, or touch a directory or an unrelated file without explicit authorization. A missing file is reported, never created.
2. **Read all of it** before editing. Mark frontmatter and protected regions; note purpose, audience, terminology. If it is too large to read completely, say so, do not claim a full review, and offer section-by-section work.
3. **Baseline.** Keep the original text (or use git) for comparison: heading hierarchy, list structure, protected blocks, links, tables, requirements, warnings, numeric claims. Create no auxiliary file in the repository.
4. **Review as a whole.** Is the purpose clear? The audience? Does the opening tell the reader what they need? Are terms consistent, claims precise, explanations repeated, tone right for the profile? Use other files only to understand a term; never change them.
5. **Rewrite selectively.** Edit only what benefits. Keep the organization, heading hierarchy and list structure. Keep every detail and caveat. Add no sections, examples or recommendations beyond what clarifies existing content. Delete nothing meaningful because it is hard to simplify.
6. **Validate** the result against the baseline: (a) protected content unchanged; (b) every requirement, constraint, warning, assumption and key explanation still present; (c) no claim stronger, weaker, broader, narrower or more certain; (d) all numbers, units, dates and versions kept; (e) headings, lists and table dimensions intact; (f) genuinely clearer; (g) no other file touched. Fix and recheck on failure. If you cannot validate safely, leave the original unchanged and say why.
7. **Write safely** (default mode only). Edit only the target, with the normal edit mechanism, avoiding partial writes. Keep its encoding and line endings. If the file has uncommitted git changes, warn and ask before overwriting them. If it changed since you read it, re-read and reconcile; never overwrite concurrent edits. No broad formatters, no staging, committing or pushing, no other files.
8. **Report** (format below).

### Review-only

Write nothing. Return: an overall assessment; the main readability problems; robotic or repetitive phrasing; jargon that needs explaining; representative before/after examples; ambiguities that should not be rewritten without clarification. Do not produce a full replacement unless asked.

### Dry-run

Do steps 1 to 6, then write nothing. Show a concise summary and, if you can produce one reliably, a unified diff. State clearly that no changes were written. Create no temporary file in the repository.

## Failure handling

| Situation | Do this |
|---|---|
| File missing | Report the path; do not create it |
| Path outside the project | Ask for authorization |
| Not Markdown | Say the skill handles Markdown only |
| Cannot read all of it | No full-review claim; propose sections |
| Malformed frontmatter | Preserve it; report it |
| Protected content cannot be preserved | Leave the file unchanged |
| Table cannot be kept intact | Leave that table unchanged; report why |
| Meaning ambiguous | Keep the original wording; report it |
| File changed meanwhile | Re-read and reconcile; never overwrite silently |
| Validation fails | Correct and recheck, or leave the original |
| `--review-only` or `--dry-run` | Never write the target |

Never replace a file with a command that discards the original without first checking the target.

## Report

End with a short summary: target path, profile, mode, the main kinds of improvement, whether protected content and technical meaning were preserved, ambiguities or content deliberately left alone, and whether the file was modified. Say what you actually checked. Do not claim the check is a mathematical guarantee.
