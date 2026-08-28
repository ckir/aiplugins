---
name: todo-conventions
description: Explains this project's TODO/FIXME/HACK marker conventions and the owner requirement. Use when writing, reviewing, or cleaning up code markers, when a hook reports an unowned marker, or when the user asks about TODO style, marker owners, or how to record deferred work.
---

# Code marker conventions

Deferred work is recorded in the code as a **marker**. A marker without an owner
is a wish; a marker with an owner is a commitment. This project only accepts the
second kind.

## The format

```
KIND(owner): note
```

- **KIND** — one of `TODO`, `FIXME`, `HACK`.
- **owner** — the person or team accountable for resolving it. Required.
- **note** — what needs to happen, in enough detail that someone else could do it.

```rust
// TODO(alice): retry on 5xx once the client exposes a retry policy
// FIXME(platform-team): this races when two writers share a connection
// HACK(bob): pinned to 0.4 until upstream fixes the panic in 0.5
```

## Choosing a kind

| Kind | Means | Typical urgency |
|---|---|---|
| `TODO` | Known future work, nothing is broken | Planned |
| `FIXME` | Something is wrong and should be corrected | Soon |
| `HACK` | A deliberate workaround that should not outlive its cause | Revisit when the cause clears |

Reach for `FIXME` only when behaviour is actually incorrect. Downgrading real
bugs to `TODO` is how they get lost.

## Writing a good note

- Say what to do, not that something is wrong: `TODO(alice): cache the parsed
  config` beats `TODO(alice): slow`.
- Name the unblocking condition when there is one: `HACK(bob): remove when
  serde 2.0 lands`.
- Link an issue when one exists. A marker is a breadcrumb, not a tracker.

## When the hook reports an unowned marker

The plugin's `PostToolUse` hook reports markers written without an owner. It
never blocks the edit — it only tells you. On seeing that report:

1. If the work is real, add the owner: `TODO:` becomes `TODO(name):`.
2. If it was a passing thought, delete it.
3. If it belongs to someone else, name them rather than yourself.

Do not silence the report by rephrasing the marker to dodge detection —
`TO-DO` and `T0DO` defeat the scanner and the convention at once.

## Turning the requirement off

Owner enforcement is per-project, in `.claude/claude-example.local.md`:

```markdown
---
require_owner: false
---
```

Prefer leaving it on. Turning it off is reasonable for a scratch repository or
a spike branch, and rarely reasonable for shared code.
