# Draft upstream report: `opencode api` loses the tail of a large piped stdout

Target: github.com/anomalyco/opencode (issues). Status: DRAFT, not filed; the owner decides whether and when.
Tracks aiplugins#102. Searched upstream issues for this on 2026-10-21: none found.

---

**Title:** `opencode api` truncates large output when stdout is a pipe (un-awaited write before `process.exit`)

**Version:** opencode v2.0.32 (source read at that tag); the same write is still on the default branch
(`packages/cli/src/commands/handlers/api.ts:46`, fetched 2026-10-21).

**Reproduce**

```
for i in $(seq 10); do opencode api GET /openapi.json | wc -c; done   # 280292 bytes expected
opencode api GET /openapi.json > out.json; wc -c out.json            # always 280292
```

Piped, the count stops at exactly 262144 with exit status 0 in 1 of 8 and 4 of 10 runs (Linux, v2.0.32).
Redirected to a file it is complete every time. The silent exit 0 is what makes it hard to notice.

**Cause (v2.0.32)**

- `packages/cli/src/commands/handlers/api.ts:45-46`: the handler reads `response.text()` and calls
  `process.stdout.write(output + ...)` once, with no callback and no drain.
- `packages/cli/src/index.ts:139`: `Effect.tap(() => Effect.sync(() => process.exit(process.exitCode ?? 0)))`
  runs right after the handler.
- Under Bun a write to a pipe larger than the pipe buffer is asynchronous, so `process.exit` discards what
  the reader has not drained. A write to a file is synchronous, hence complete. 262144 is the pipe buffer
  size, not a number in the source.

**Suggested fix:** wait for the write before exiting, e.g.
`yield* Effect.promise(() => new Promise<void>((done) => process.stdout.write(out, () => done())))`
in the handler, or have the exit tap wait for stdout to drain.

**Workaround we use:** redirect stdout to a file, never a pipe.
