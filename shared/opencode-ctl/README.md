# opencode-ctl

Shared engine crate for the opencode-ctl MCP servers. It controls a local
`opencode` server on behalf of a host agent: starting, pairing and attaching
sessions, relaying requests to the opencode HTTP API, and cleaning up stale
state.

Several per-host front crates (Claude, Qwen, Antigravity) wrap this engine; it
ships no binary of its own to users. The only binary target, `mock-opencode-ctl`,
is a test double used by the integration tests.

Layout:

- `error`: the stable `ErrorCode` set and `ToolError` envelope.
- `host`: which host identity is running and what it may do.
- `clock`: the `Clock` seam and the deterministic `TestClock`.
- `liveness`: `ProcessProbe`, the process-identity checks used for pairing and gc.
- `cli`: argv routing and the dispatch skeleton.
- `testing`: support shared with the fronts (`FakeProbe`, temp homes, env maps).

Run the tests with `cargo test -p opencode-ctl`.
