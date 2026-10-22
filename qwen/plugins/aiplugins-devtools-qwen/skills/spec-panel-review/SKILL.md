---
name: spec-panel-review
description: Standalone, report-only adversarial multi-seat review of a spec or implementation plan (not code), run inline without clavity. Use for a "panel review" or "spec panel" of a design doc, spec or plan before anyone builds from it.
---

# Spec Panel Review

Tear down a spec or plan with a panel of seats, each hunting a different class of defect, before anyone builds from it. Report-only: the artifact is never edited unless the user asks.

Not for code or diffs; use `code-review-and-quality` for those.

## Input

The target is the path given as the argument. If none was given, ask for it. Read the whole file before doing anything else. If the file does not exist or is not a spec, plan or similar document, say so and stop.

## Step 1: Select the stance and the seats

Pick one stance:

- `relentless-adversarial-auditor`: default, for an artifact still under construction.
- `merge-gate-adversarial-auditor`: a final gate whose job is to block the build if the artifact does not hold up.
- A sharper bespoke stance when the artifact plainly calls for one (a performance skeptic, a crypto auditor).

Always seat both core seats. Seat every specialist whose use-when condition the artifact actually meets, and drop the rest.

**Core seats (always):**

- **Axiom Breaker**: contradictory constraints, circular logic, unstated invariants or assumptions, scope creep, decisions that will not survive first real use.
- **Cascade Analyst**: unhandled failure paths, silent drops, errors that compound or leak once something upstream fails or hangs.

**Specialist seats:**

- **State Corruptor**: data races, cache staleness, idempotency failures, out-of-order events. Use when the artifact manages state, concurrency, async flows or caching.
- **Boundary Smuggler**: trust bypass, injection, payload spoofing, unauthorized mutation. Use when it crosses a trust zone, handles auth or reads untrusted input.
- **Resource Vampire**: unbounded queues, handle or connection leaks, un-paginated data, quota exhaustion. Use when it iterates collections, makes network calls, allocates resources or drives repeated LLM or API calls.
- **Protocol Pedant**: schema mismatch, serialization loss, silent truncation, rigid parsing. Use when it defines or consumes a wire contract, API, data schema or CLI signature.
- **Blindspot Auditor**: misleading logs, irreversible destructive footguns, missing observability. Use when a human must configure, deploy, operate or debug it, especially during an outage.
- **Dependency Cynic**: upstream drift, brittle version pinning, missing lockfiles, environment assumptions. Use when it adds a library or toolchain or relies on local machine state.
- **Literal Implementer**: hand-wavy instructions, deferred "TBD" decisions, un-sized placeholders, steps that force the reader to guess. Use when it is a spec, plan, checklist or guide meant for an agent or human to execute.
- **Activation Auditor**: broken globs, regex misfires, vague or over-eager descriptions, triggers that never fire or fire constantly. Use when it defines a skill, hook, CI trigger, CLI command or other auto-discovered component.
- **Mechanism Gamer**: rules or gates that can be satisfied without producing the intended effect: bypassable checks, gameable quotas, false-GREEN outcomes, compliance theater. Use when it defines a rule, gate, quota or process someone must follow.

Three to six seats is typical, not a cap. When more conditions fire, seat them all; never drop a seat whose condition really fires to stay small. A bespoke seat is allowed when the artifact needs a lens the palette lacks.

Before convening, write down which use-when conditions fired and which specialists you dropped and why. A panel of only the two core seats on a rich artifact is under-seated and cannot support a GREEN verdict.

## Step 2: Convene the seats

**If you have a subagent tool**, dispatch one fresh-context, read-only subagent per seat, in parallel. Each gets only: the target path, its single seat definition, the stance, and the output rules below. Give it no session history, no author rationale and no other seat's output. A seat that shares your context repeats your reasoning; a fresh one can contradict it.

**If you have no subagent tool**, run the seats one after another in this conversation, each under its own heading as working notes; Steps 3 to 5 turn them into the report. State plainly in the report that the seats shared one context, so their findings are correlated and the verdict is weaker than a fresh-context panel would give. Do not let a seat repeat a defect an earlier seat already claimed.

Output rules for every seat:

- Each finding quotes the exact line of the artifact (or names the missing section) and states the failure mechanism and its consequence.
- A seat with nothing new writes `no new findings` and stops. Padding with restated or trivial points is a defect.
- Findings are claims, not facts. Do not trust a seat's description of code or tools outside the artifact.

## Step 3: Verify before you report

Do not forward a claim you have not checked. For every finding, and separately for every fix a seat proposes:

- Check each factual claim about the repository, a tool, an API or a file by reading or searching it. Quote what you found.
- A finding can be right while its proposed fix is wrong or incomplete. Check the fix as its own claim, including what it would break.
- A finding can be right while the mechanism or number attached to it is invented. Keep the defect, correct the specifics, and never copy unverified numbers into the report.

## Step 4: Disposition every finding

Every finding ends in exactly one of these. There is no "noted".

- `CONFIRMED: <what is wrong and what must change>`
- `REFUTED: <the quoted line or tool output that kills it>`
- `BELOW-FLOOR: <target> unreachable because <guard at file:line>`: contrived or unreachable. Cite the guard; prose alone is not enough.
- `DEFERRED: <file:line or n/a>`: reachable, real, not for this artifact to fix. Allowed only for a defect that existed before the artifact and that the artifact does not cause.
- `UNVERIFIED: <finding>`: neither provable nor refutable from what you can read.

A defect's age is never a disposition: "pre-existing" and "out of scope" are not reasons. Severity floor: stylistic or trivial observations that touch no correctness, safety, contract or completeness question are dropped, not listed.

## Step 5: Report

Write the report in the conversation, in this order:

1. Stance, seated seats, which use-when conditions fired, dropped seats and why, and whether the seats were fresh-context or shared-context.
2. Each seat's findings under its own heading, each with its disposition. `BELOW-FLOOR` findings are not repeated here; they go in item 3.
3. The one or two most significant `BELOW-FLOOR` items you dropped, each with its cited guard, so a real defect cannot be swept under the floor unseen. Count any others without listing them.
4. What was not examined: files unread, behaviour never exercised, lenses not applied. A round that finds nothing has told you about its coverage, not about the artifact.
5. One closing line: `PANEL VERDICT: GREEN`, `REVISE` or `BLOCK`, then a short reason.
   - `GREEN` only when no `CONFIRMED` and no `UNVERIFIED` finding remains and the seated lenses actually cover the artifact's risk surface. Name any `DEFERRED` finding in the reason.
   - `REVISE` when `CONFIRMED` findings can be fixed in the artifact, or `UNVERIFIED` findings need the author to settle them.
   - `BLOCK` when a `CONFIRMED` finding challenges a decision the artifact cannot proceed without.

A panel reasons over the artifact as written; it does not run anything. Follow a GREEN with the artifact's own oracle (the tests, the build, the real tool) before relying on it.

## After the report

Do not edit the spec or plan. If the user asks to fold the confirmed findings into it, apply only the `CONFIRMED` ones, then run a fresh review of the edited file as a new invocation. Do not loop automatically: each new pass is the user's call.
