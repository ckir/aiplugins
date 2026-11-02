# MVP Implementation Specification: Human-Friendly Markdown Editor Skill

**Target repository:** https://github.com/ckir/aiplugins  
**Feature:** `human-friendly`  
**Distribution:** Existing `aiplugins-devtools` skill packages  
**Target agents:** Claude Code, Antigravity, Qwen Code, OpenCode  
**Document type:** Coding-agent implementation handoff  
**Status:** Ready for implementation  
**Scope:** MVP — a reusable agent skill that reviews and rewrites Markdown files in place

---

## 1. Mission

Implement a new reusable skill named `human-friendly` in the existing `aiplugins` repository.

The skill enables a coding agent to review a Markdown document in the user's repository and rewrite it so that it reads naturally, clearly, and professionally, while preserving its technical meaning and important document structure.

The skill must be distributed through the repository's existing `aiplugins-devtools` plugin family. It must follow the current shared-skill workflow rather than introducing a separate distribution mechanism.

### 1.1 Core user experience

A user should be able to invoke the skill with a Markdown file path, for example:

```text
/human-friendly README.md
/human-friendly docs/architecture.md
/human-friendly docs/specification.md
```

The agent should:

1. Read the requested file.
2. Understand its purpose, audience, and technical subject.
3. Review its writing quality.
4. Rewrite only the parts that benefit from improvement.
5. Preserve technical meaning and protected content.
6. Validate the result against the original.
7. Write the improved content back to the requested file.
8. Report what changed and any unresolved ambiguity.

The default operation is **rewrite in place**.

The skill must also support review-only operation, in which it reports findings without modifying the file.

### 1.2 Product principle

This is a writing-quality skill, not an AI detector and not a tool for disguising AI-generated text.

The objective is clear, natural, human-readable writing. Do not promise that a document will evade AI detectors or claim that authorship can be determined reliably.

---

## 2. Repository integration requirements

The target repository is a multi-agent plugin marketplace. Its existing shared developer-tools implementation is the canonical integration point.

Before editing, the implementation agent MUST inspect the current state of the repository and confirm all paths, naming conventions, scripts, manifests, and release wiring. This specification describes the observed architecture but must not override newer repository conventions.

### 2.1 Canonical source

Create:

```text
shared/
  devtools/
    skills/
      human-friendly/
        SKILL.md
```

`shared/devtools/skills/human-friendly/SKILL.md` is the single source of truth for the skill.

Use the existing skill's frontmatter conventions. The frontmatter must contain:

- `name: human-friendly`
- A concise description that clearly explains when the skill should be used.

Keep the description concise because skill frontmatter contributes to resident context cost.

### 2.2 OpenCode command

Inspect the existing files in:

```text
shared/devtools/commands/
```

Add:

```text
shared/devtools/commands/human-friendly.md
```

Follow the existing command format exactly:

- Include the required `description:` frontmatter.
- Refer to the installed skill using the established prefixed name:
  `aiplugins-devtools-human-friendly`.
- Pass the user's target path and options through `$ARGUMENTS`.
- Do not duplicate the full skill instructions in the command file.

OpenCode needs this additional command file because the repository's existing OpenCode integration does not expose skills in its slash-command menu in the same way as the other supported agents.

### 2.3 Generated distribution copies

Do not manually create or edit generated copies.

Use the repository's existing emitter:

```bash
just emit-devtools-skills
```

The emitter should discover the new skill and copy it into the four existing `aiplugins-devtools` plugin packages:

- `claude-code/plugins/aiplugins-devtools-cc`
- `antigravity/plugins/aiplugins-devtools-agy`
- `qwen/plugins/aiplugins-devtools-qwen`
- `opencode/plugins/aiplugins-devtools-opencode`

For OpenCode, verify that the emitter applies the expected prefixed skill directory and frontmatter name transformation, and copies the new command file.

Do not change the emitter unless the existing implementation demonstrably fails to handle the new skill.

### 2.4 Existing package and release metadata

Inspect, but do not unnecessarily modify:

- `agents.json`
- `.release-please-manifest.json`
- `release-please-config.json`
- `.claude-plugin/marketplace.json`
- Agent-specific plugin manifests
- Skill bundle workflows
- Skill drift checks
- Footprint budgets

The new skill belongs to the existing `aiplugins-devtools` packages. Do not create a fifth package or a new marketplace entry.

If the existing release configuration automatically discovers the new skill through the existing package, leave it unchanged.

Do not create or push release tags. Do not change package versions as part of this feature unless the repository's established contribution workflow explicitly requires a version update in source control.

---

## 3. Skill behavior and user interface

### 3.1 Invocation

The primary invocation is:

```text
/human-friendly <markdown-file>
```

Examples:

```text
/human-friendly README.md
/human-friendly docs/design.md
/human-friendly docs/architecture.md
```

The skill should also understand natural-language requests such as:

- "Make this document easier to read."
- "Review and humanize this Markdown file."
- "Rewrite this specification in natural professional language."
- "Improve the wording without changing the technical content."

The skill must not require the user to specify a writing profile for ordinary use.

### 3.2 MVP modes

Support these modes:

| Mode | Behavior | Writes file? |
|---|---|---:|
| Default rewrite | Review, improve and validate the document | Yes |
| `--review-only` | Report findings and suggested improvements | No |
| `--dry-run` | Produce a proposed rewrite/report without changing the target | No |
| `--profile natural-professional` | Use the default professional profile explicitly | Yes, unless review-only/dry-run |
| `--profile conversational` | Use a more relaxed, personal style | Yes, unless review-only/dry-run |
| `--profile executive` | Use concise, decision-oriented business writing | Yes, unless review-only/dry-run |

If a mode or option is unsupported or ambiguous, explain it rather than silently guessing.

Do not implement a complex option parser or separate executable for the MVP. These are agent-interpreted instructions in the skill.

### 3.3 Profile definitions

**Natural Professional (default)**

- Clear, natural, professional, and approachable.
- Appropriate for READMEs, technical documentation, architecture documents, specifications, and project documentation.
- Direct language without unnecessary informality.
- Retains necessary technical vocabulary.

**Conversational**

- More relaxed and personal.
- Appropriate for blog posts, community documentation, tutorials, and explanatory articles.
- Still accurate, respectful, and well structured.
- Never forced, slang-heavy, or artificially enthusiastic.

**Executive**

- Concise and decision-oriented.
- Appropriate for proposals, reports, and business documents.
- Emphasizes outcomes, decisions, risks, and actions.
- Does not remove caveats or technical constraints.

If no profile is provided, use Natural Professional.

---

## 4. Writing quality requirements

The skill must behave like an experienced human technical editor.

### 4.1 Natural language

- Prefer familiar words over unnecessarily sophisticated vocabulary.
- Prefer direct statements over indirect or inflated phrasing.
- Use active voice when it improves clarity.
- Keep sentences reasonably short.
- Vary sentence structure naturally.
- Avoid repetitive sentence openings.
- Keep paragraphs focused on one main idea.
- Use transitions only when they help the reader follow the argument.
- Preserve the author's intended voice where it is already effective.

### 4.2 Clarity

- Explain complex concepts in simpler language where possible.
- Introduce technical concepts before relying on them.
- Explain unavoidable jargon when the intended audience may not know it.
- Use concrete examples only when they improve understanding.
- Make requirements, warnings, decisions, and actions easy to identify.
- Prefer precise statements over vague generalities.

### 4.3 Remove writing problems

Look for and improve:

- Robotic or unnatural phrasing.
- Overly long or convoluted sentences.
- Unnecessary jargon.
- Repetition and redundant explanations.
- Filler words and empty introductions.
- Generic conclusions that add no information.
- Excessive formality.
- Unnecessary passive voice.
- Awkward transitions.
- Inconsistent terminology.
- Unsubstantiated marketing language.
- Excessive adjectives and intensifiers.

### 4.4 Avoid generic AI-writing patterns

Avoid introducing or retaining formulaic language when it adds no meaning, including:

- "In today's rapidly evolving technological landscape..."
- "It is important to note that..."
- "Furthermore..." and "Moreover..." used mechanically.
- "Let's dive into..."
- "Unlock the power of..."
- "Seamlessly" when no meaningful seamless behavior is being described.
- "Robust and scalable" without concrete justification.
- Generic "In conclusion" paragraphs that merely repeat the document.

This is a guidance list, not a banned-word list. Do not remove a phrase when it is genuinely appropriate.

### 4.5 Do not over-edit

- Do not rewrite sentences merely to make them different.
- Keep existing wording when it is already clear and natural.
- Do not shorten a document at the expense of important detail.
- Do not make technical writing sound like marketing copy.
- Do not make professional writing artificially casual.
- Do not introduce intentional errors or awkwardness to imitate human writing.

---

## 5. Technical integrity and protected content

This section is mandatory and takes precedence over stylistic preferences.

The skill must preserve the meaning and operational content of the original document.

### 5.1 Content that must remain unchanged

Unless the user explicitly requests otherwise, preserve exactly:

- Fenced code blocks and their contents.
- Inline code spans, including identifiers and commands.
- Shell commands and command-line options.
- API names, symbols, identifiers, and function names.
- Configuration keys and values.
- File paths and filenames.
- URLs and link destinations.
- Reference-style link definitions and their identifiers.
- Image destinations and alt text.
- Markdown tables' data and column relationships.
- YAML frontmatter and its values.
- Embedded HTML and its attributes.
- Mermaid diagrams and diagram source.
- Math expressions and formulas.
- License text and legal notices.
- Citations and source references.
- Version numbers and compatibility statements.
- Requirements, constraints, assumptions, and acceptance criteria.
- Security warnings and safety instructions.
- Numeric values, units, dates, and thresholds.

Do not translate, rename, normalize, or "correct" protected identifiers.

### 5.2 Content that may be edited

The skill may improve ordinary prose, including:

- Headings, provided their meaning and hierarchy remain intact.
- Paragraphs.
- List-item prose.
- Table-cell prose, without changing data or relationships.
- Blockquote prose, without changing quoted source material.
- Image alt text only when the user explicitly authorizes it; otherwise preserve it.
- Captions and explanatory text.

For quoted text, preserve direct quotations and their attribution. Do not paraphrase a quotation as if it were the original source's wording.

### 5.3 YAML frontmatter

Treat YAML frontmatter as machine-readable metadata.

Default behavior:

- Preserve the complete frontmatter block byte-for-byte.
- Do not change keys, values, ordering, quoting, or delimiters.
- Do not interpret frontmatter prose as ordinary document prose.
- If frontmatter is malformed or its boundaries are unclear, do not attempt to repair it. Report the issue.

### 5.4 Code and structured blocks

Treat all fenced code blocks as protected, regardless of language.

Also protect:

- Indented code blocks.
- Mermaid blocks.
- JSON, YAML, TOML, XML, and other structured examples.
- Embedded scripts and HTML.
- Math blocks.

Do not reformat code, change indentation, or normalize line endings inside protected blocks.

Inline code spans must remain exact.

### 5.5 Tables

Table prose may be rewritten, but:

- Preserve the number and order of rows and columns.
- Preserve every data value.
- Preserve alignment markers.
- Preserve links and inline code.
- Do not convert a table into a list or vice versa.
- If the table is malformed or cannot be safely edited, leave it unchanged and report why.

### 5.6 Semantic preservation

Never change the strength or scope of a claim.

Examples:

Original:

> The application supports providers A and B.

Safe paraphrase:

> You can use providers A and B with the application.

Unsafe paraphrase:

> The application supports all major providers.

Original:

> The operation may fail if the network connection is interrupted.

Unsafe paraphrase:

> The operation will recover automatically if the network connection is interrupted.

Preserve distinctions such as:

- must / should / may
- always / sometimes / never
- supported / planned / experimental
- required / optional
- possible / guaranteed
- current behavior / future intent

Never invent features, guarantees, performance claims, compatibility, or evidence.

If the original is ambiguous, preserve the ambiguity and report it. Do not silently resolve it by guessing.

---

## 6. Required execution workflow

The skill instructions must implement the following sequence.

### Step 1 — Resolve the target

- Identify the Markdown file supplied by the user.
- Resolve relative paths from the agent's current project/repository context.
- Confirm that the target exists and is a regular file.
- Confirm that the target is a Markdown document (`.md` or `.markdown`).
- Do not follow a path outside the user's intended project without explicit authorization.
- Do not overwrite a directory, symlink target outside the intended project, or unrelated file.
- If no target is supplied and no unambiguous target is available from context, ask one concise question.
- If multiple files are supplied, process each independently and report each result.

Do not search and rewrite an entire repository when the user supplied one file.

### Step 2 — Read the complete document

- Read the complete target file before editing.
- Do not base a rewrite on snippets or partial context.
- Identify frontmatter and protected regions.
- Understand the document's purpose, audience, terminology, and structure.
- If the file is too large to read completely within available context, do not pretend it was fully reviewed. Explain the limitation and propose safe section-by-section processing.

### Step 3 — Establish a baseline

Record the original content or use the agent's available file/diff capabilities to preserve a comparison baseline.

Identify:

- Heading hierarchy.
- Paragraph and list structure.
- Protected code and structured blocks.
- Links and references.
- Tables and their data.
- Requirements, constraints, warnings, and numeric claims.

Do not create permanent auxiliary files in the repository for this baseline.

### Step 4 — Review the writing

Review the document as a whole before editing individual paragraphs.

Consider:

- Is the purpose clear?
- Is the intended audience clear?
- Does the introduction explain what the reader needs to know?
- Are the paragraphs easy to follow?
- Are sentences unnecessarily complicated?
- Is terminology consistent?
- Are there repeated explanations?
- Are claims precise and supported by the original document?
- Does the tone match the selected profile?

Do not invent context from other repository files. Other files may be consulted only when needed to understand a reference or technical term, and must not be treated as authorization to change them.

### Step 5 — Rewrite selectively

- Rewrite only content that benefits from editing.
- Preserve the original document's organization unless a structural issue prevents comprehension.
- Keep heading hierarchy and list structure.
- Retain all technical details and caveats.
- Do not add new claims, sections, examples, or recommendations unless necessary to clarify existing content without changing its meaning.
- Do not delete meaningful content simply because it is difficult to simplify.

### Step 6 — Validate the rewrite

Compare the proposed result with the original.

Perform these checks:

1. **Protected content:** Verify that frontmatter, code, inline code, links, URLs, structured examples, formulas, citations, and other protected content are unchanged.
2. **Completeness:** Verify that every original requirement, constraint, warning, assumption, and important explanation remains represented.
3. **Meaning:** Verify that no claim has become stronger, weaker, broader, narrower, or more certain.
4. **Numbers:** Verify that all numbers, units, dates, versions, and thresholds are preserved.
5. **Structure:** Verify that heading hierarchy, list relationships, table dimensions, and document organization remain valid.
6. **Readability:** Verify that the result is genuinely clearer and more natural.
7. **Scope:** Verify that no unrelated file was changed.

If any validation check fails, correct the rewrite before writing the file.

If safe validation is not possible, do not silently proceed. Explain the limitation and leave the original unchanged.

### Step 7 — Write safely

For default rewrite mode:

- Modify only the requested Markdown file.
- Use the agent's normal safe file-editing mechanism.
- Avoid partial writes that could leave the file corrupted.
- Preserve the original file's encoding and line-ending convention where possible.
- Do not run broad formatters or repository-wide cleanup.
- Do not stage, commit, or push changes.
- Do not change other files.

If the target has changed since it was read, re-read it and reconcile before writing. Do not overwrite concurrent user edits.

### Step 8 — Report

Return a concise summary containing:

- Target file path.
- Selected profile.
- Mode used.
- Main categories of improvements.
- Whether technical content and protected sections were preserved.
- Any ambiguities or content deliberately left unchanged.
- Whether the file was modified.

Do not claim that automated semantic verification is mathematically guaranteed. Report what was actually checked.

---

## 7. Review-only and dry-run behavior

### 7.1 `--review-only`

Do not modify any file.

Return:

1. Overall writing assessment.
2. Main readability issues.
3. Repetitive or robotic phrasing.
4. Technical language that may need explanation.
5. Representative before/after examples.
6. Any ambiguities that should not be rewritten without clarification.

Do not produce an entire replacement document unless the user asks for one.

### 7.2 `--dry-run`

- Read and review the target.
- Prepare the proposed rewrite.
- Validate it against the original.
- Do not write the target file.
- Show a concise summary and a unified diff if the agent can produce one reliably.
- Clearly state that no changes were written.

Do not create a temporary file in the repository unless the user explicitly asks for one.

---

## 8. Failure handling

The skill must stop safely and explain the problem when:

| Situation | Required behavior |
|---|---|
| File does not exist | Report the missing path; do not create it |
| Path is outside intended project | Ask for authorization |
| File is not Markdown | Explain that the MVP supports Markdown only |
| File cannot be read completely | Do not claim a full review; propose section-based work |
| Malformed frontmatter | Preserve it and report the issue |
| Protected content cannot be preserved safely | Leave the file unchanged |
| Table structure cannot be preserved | Leave that table unchanged and report it |
| Technical meaning is ambiguous | Preserve original meaning/wording and report ambiguity |
| File changed concurrently | Re-read and reconcile; never overwrite silently |
| Validation fails | Correct and revalidate, or leave the original unchanged |
| Review-only/dry-run requested | Never write the target |

The skill must not use shell commands that destructively replace files without checking the target and preserving the original content.

---

## 9. Skill file content requirements

`shared/devtools/skills/human-friendly/SKILL.md` must be a concise but complete operational instruction document.

It must contain:

1. Valid skill frontmatter.
2. Purpose and invocation guidance.
3. Supported modes and profile selection.
4. Writing principles.
5. Technical preservation rules.
6. The complete execution workflow.
7. Validation requirements.
8. Safe-write requirements.
9. Failure handling.
10. Final reporting format.

Keep the skill focused. Do not add large generic writing essays, redundant examples, or lengthy explanations that consume context without changing agent behavior.

The skill must be self-contained at invocation time. It must not depend on the user having read this implementation specification.

---

## 10. OpenCode command requirements

Create `shared/devtools/commands/human-friendly.md` using the exact conventions of existing commands.

The command should:

- Have a short description.
- Invoke `aiplugins-devtools-human-friendly`.
- Pass `$ARGUMENTS` without dropping the target path or mode flags.
- Avoid duplicating the skill's full behavior.
- Work when the user invokes `/human-friendly README.md`.

Follow existing naming, frontmatter, and command-body conventions rather than inventing a new command format.

---

## 11. Tests and repository validation

This feature is primarily an instruction-based skill. Do not introduce a runtime, MCP server, library, binary, or external service for the MVP.

### 11.1 Required repository checks

Run the repository's existing checks, including:

```bash
just emit-devtools-skills
just devtools-skills
just skills-bundles
just footprint-regen
```

Inspect `justfile` and the relevant scripts first. If a command has side effects, understand them before execution.

Run the relevant existing repository checks as appropriate, including:

- Formatting and lint checks.
- Plugin manifest validation.
- Skill drift checks.
- Bundle validation.
- Release wiring checks.
- Footprint budget checks.

At minimum, verify:

- The canonical skill exists.
- The skill frontmatter name is exactly `human-friendly`.
- The OpenCode command exists and follows the expected naming convention.
- All four generated skill copies exist.
- The generated copies match the canonical skill according to the repository's drift-check rules.
- OpenCode's generated skill name and command references are correct.
- Skill bundles include the new skill.
- Existing skills and commands have not been unintentionally modified.
- The footprint budget check passes.

Do not bypass or weaken repository checks to make the feature pass.

### 11.2 Behavioral test cases

The coding agent must validate the skill instructions against the following scenarios. These can be manual agent tests; do not add a test framework solely for this feature.

**Test A — README prose**

Input:

```markdown
# Example Tool

This application provides users with the ability to execute
multiple operations in an efficient and streamlined manner.

The tool supports Linux and Windows.
```

Expected:

- Natural, direct wording.
- No invented capabilities.
- Linux and Windows support remains explicit.
- Heading remains intact.

**Test B — Protected code**

Input:

````markdown
# Installation

The following command can be used to install the package.

```bash
npm install example-package
```
````

Expected:

- Explanatory prose may improve.
- The fenced block remains exactly unchanged.
- The command is not reformatted or renamed.

**Test C — Technical requirement**

Input:

```markdown
The operation MUST fail if the lock cannot be acquired.
The operation MAY retry after a transient network failure.
```

Expected:

- MUST and MAY semantics remain unchanged.
- No automatic retry guarantee is introduced.
- Both requirements remain explicit.

**Test D — Links and references**

Input:

```markdown
Read the [installation guide](https://example.com/install)
for additional information.
```

Expected:

- Prose may improve.
- Link text may be improved only if its meaning remains equivalent.
- Destination URL remains byte-for-byte identical.

**Test E — YAML frontmatter**

Input:

```yaml
---
name: example
version: "1.2.3"
description: Example plugin
---
```

Expected:

- Entire frontmatter block remains unchanged.
- No key, value, quoting, or ordering changes.

**Test F — Table**

Input:

```markdown
| Feature | Status |
|---|---|
| Linux | Supported |
| Windows | Experimental |
```

Expected:

- Table dimensions and status values remain unchanged.
- Prose may improve, but Experimental must not become Supported.

**Test G — Review-only**

Expected:

- Findings and examples are returned.
- Original file remains unchanged.

**Test H — Dry-run**

Expected:

- Proposed changes and, where available, a diff are returned.
- Original file remains unchanged.

**Test I — Already clear writing**

Expected:

- Clear prose is left alone.
- The agent does not rewrite every sentence just to produce visible changes.

**Test J — Ambiguous claim**

Input:

```markdown
The system may support recovery after a process failure.
```

Expected:

- The uncertainty represented by "may" remains.
- The rewrite does not promise recovery.
- Ambiguity is reported if the intended behavior cannot be established.

**Test K — Scope**

Expected:

- Only the specified Markdown file is modified.
- No unrelated repository files are changed by skill execution.

---

## 12. Documentation

Update the existing `shared/devtools/README.md` only if its documented skill inventory or maintainer instructions require it.

Update the relevant `aiplugins-devtools` plugin README files if they maintain an explicit list of included skills or user-facing examples. Do not edit all four READMEs blindly; inspect their current content first.

The user-facing documentation should explain:

- What Human-Friendly does.
- How to invoke it.
- Default rewrite behavior.
- Review-only and dry-run options.
- Available writing profiles.
- Its commitment to preserving technical meaning and protected Markdown content.

Update the root README only if the repository's established conventions require listing every skill there.

Do not add a new standalone documentation site or a large tutorial for this MVP.

---

## 13. Explicit non-goals

Do not implement these in the MVP:

- A standalone web UI.
- A desktop application.
- A new MCP server.
- A separate model/provider integration.
- Model selection or model routing.
- Automatic AI-generated-text detection.
- AI-detector evasion or "humanization score".
- A readability scoring engine.
- A grammar-checking engine.
- A Markdown AST transformation framework.
- Automatic repository-wide documentation rewriting.
- Batch processing of arbitrary directories.
- Automatic translation.
- Automatic fact-checking against the web.
- Automatic technical correction.
- Git commit, push, or pull-request creation.
- A new plugin package or marketplace entry.
- A persistent user profile or configuration database.

The skill uses the model and file-editing capabilities of the host coding agent.

---

## 14. Implementation plan

Implement in small, reviewable phases.

### Phase 1 — Repository reconnaissance

1. Inspect `shared/devtools/README.md`.
2. Inspect the existing canonical skill under `shared/devtools/skills/`.
3. Inspect existing OpenCode command files.
4. Inspect `just emit-devtools-skills`.
5. Inspect `scripts/check-devtools-skills.sh`.
6. Inspect skill bundling and footprint checks.
7. Inspect relevant plugin READMEs and manifests.
8. Record any differences between the observed repository state and this specification.

Do not edit files during reconnaissance.

### Phase 2 — Canonical skill

1. Create `shared/devtools/skills/human-friendly/SKILL.md`.
2. Follow the current frontmatter convention.
3. Implement all mandatory behavior in this specification.
4. Keep the skill concise and operational.
5. Review the instructions for contradictions and unnecessary token cost.

### Phase 3 — OpenCode command

1. Create `shared/devtools/commands/human-friendly.md`.
2. Follow the existing command convention.
3. Confirm that it forwards `$ARGUMENTS`.
4. Confirm that it refers to `aiplugins-devtools-human-friendly`.

### Phase 4 — Generate distribution copies

Run:

```bash
just emit-devtools-skills
```

Inspect the resulting changes. Do not manually patch generated copies.

### Phase 5 — Documentation

Update only documentation that needs an explicit inventory or user-facing usage entry.

### Phase 6 — Validation

Run:

```bash
just devtools-skills
just skills-bundles
just footprint-regen
```

Run other relevant checks discovered during reconnaissance.

Perform the behavioral test cases in Section 11.2 to the extent supported by the available coding-agent environments.

### Phase 7 — Final review

Inspect the complete diff.

Confirm:

- Only intended source, generated, and documentation files changed.
- No unrelated skill behavior changed.
- No generated drift remains.
- No release tags or versions were changed unnecessarily.
- No tests or checks were weakened.
- No temporary files remain.

---

## 15. Acceptance criteria

The feature is complete only when all applicable criteria pass.

### Functional

- [ ] A canonical `human-friendly` skill exists under `shared/devtools/skills/`.
- [ ] The skill can be invoked with a Markdown file path.
- [ ] Default invocation rewrites the target file in place.
- [ ] `--review-only` does not modify the target.
- [ ] `--dry-run` does not modify the target.
- [ ] Natural Professional is the default profile.
- [ ] Conversational and Executive profiles are supported.
- [ ] The skill avoids unnecessary rewrites.
- [ ] The skill reports unresolved ambiguity.

### Preservation and safety

- [ ] YAML frontmatter is preserved.
- [ ] Fenced and indented code blocks are preserved.
- [ ] Inline code is preserved.
- [ ] URLs and link destinations are preserved.
- [ ] Table structure and data are preserved.
- [ ] Requirements and modal language retain their meaning.
- [ ] Numeric values, units, and versions are preserved.
- [ ] The skill modifies only the requested file.
- [ ] Concurrent changes are not silently overwritten.
- [ ] Failed validation results in no unsafe write.

### Distribution

- [ ] OpenCode command source exists.
- [ ] All four agent packages receive the skill.
- [ ] OpenCode command and prefixed skill naming are correct.
- [ ] Existing emitter and drift checks pass.
- [ ] Skill bundles contain the new skill.
- [ ] Existing package and release wiring remains valid.

### Quality

- [ ] The skill is self-contained.
- [ ] Instructions are clear and internally consistent.
- [ ] The skill frontmatter remains concise.
- [ ] No unnecessary runtime or dependencies were introduced.
- [ ] Documentation is updated where necessary.
- [ ] The final diff has been reviewed.

---

## 16. Final report required from the coding agent

When implementation is complete, provide:

1. **Implementation summary** — what was added.
2. **Files changed** — canonical source, OpenCode command, generated copies, documentation.
3. **Distribution** — confirmation of the four supported agent packages.
4. **Validation** — commands executed and their actual results.
5. **Behavioral tests** — scenarios tested and outcomes.
6. **Footprint impact** — result of the repository's footprint check.
7. **Known limitations** — anything not tested or not safely guaranteed.
8. **Git status** — changed files and confirmation that no commit or push was performed.

Do not claim that a command passed unless it was actually executed successfully.

If a check cannot be run, report it explicitly and explain why.

---

## 17. Instructions to the implementation agent

You are implementing this feature in the existing `ckir/aiplugins` repository.

Treat this document as the feature specification.

Important execution rules:

- Start with repository reconnaissance.
- Read the existing implementation before changing it.
- Follow current repository conventions over assumptions in this document if the repository has evolved; explain any material deviation.
- Implement the smallest complete solution.
- Use the existing shared devtools skill distribution mechanism.
- Do not create a new plugin package.
- Do not add a runtime, binary, MCP server, or external dependency.
- Do not change unrelated files.
- Do not weaken checks or hide failures.
- Do not create or push release tags.
- Run the required checks and inspect the final diff.
- Continue through implementation and validation; do not stop after producing a plan.

**Definition of done:** The new `human-friendly` skill is available through the existing four `aiplugins-devtools` integrations, its OpenCode command works according to repository conventions, and all applicable repository checks pass.
