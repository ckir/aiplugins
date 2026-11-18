# repo-tasks: one task list per repository, shared by every agent

Date: 2026-11-18. Status: DRAFT for owner approval. Branch: `docs/repo-tasks-design`.

Review trail (local, gitignored, not durable; everything that matters is copied into this file):
`.clavity/seams/tasks-tracker-first.md`, `tasks-tracker-first-r2.md`,
`tasks-tracker-agreement-first.md`, `-first-r2.md`, `-first-r3.md`, and
`.clavity/scratch/tasks-tracker-agreement/`.

Three kinds of statement appear below and are kept apart:

- **[owner]** a sentence the owner wrote, quoted verbatim.
- **[approved]** a point of the text the owner approved on 2026-11-18 (section 6).
- **[driver's call]** a decision the author made to carry a point out. The owner has not ruled
  on these; each can be overruled. They are listed together in section 17.3.

## 1. Intent

A task tracker in the style of Claude Code's Tasks (each task its own record with an id, changed
one at a time, with dependencies, kept on disk), built so that the list belongs to the repository
and not to a session or an agent. Any of four agents (Claude Code, Qwen, Antigravity, OpenCode)
opened in the same repository reads and writes the same list. The team name for it is
**repo-tasks** [driver's call].

What the owner asked for, in the owner's words:

- [owner] "Build a TodoWrite replacement like Tasks"
- [owner] "Claude Code for me, ship for all four"
- [owner] "same list for any agent" (the answer to: a session dies halfway through a plan; should
  the next session, possibly a different agent, see the same list)
- [owner] "I'm going to ship it as well"

The owner gave no reason beyond these sentences, and this document does not supply one.

## 2. Who it is for, and the success criterion

The owner, working in Claude Code, and people who install plugins from this marketplace into any
of the four agents.

Success criterion (written by the author from the owner's third sentence; the peer reviewed
against it): start a five-task plan in agent A in repository R. Kill the session after task 2.
Open a different agent B in R. B's first list call returns the same five tasks with the same ids,
statuses and dependencies, with no copy step. Two agents writing at the same moment never lose an
update.

## 3. Goals, non-goals, anti-goals

Goals:

1. One list per repository, the same for every agent and every worktree of that repository.
2. The list outlives the session that wrote it.
3. Tasks are records with stable ids, changed one at a time, with dependencies.
4. It ships as a plugin for all four agents through this repository's normal release path.
5. Its resident context cost stays small (section 13).

Non-goals for the first release [approved, point 13]: sharing a list between machines or through
git; priorities; a cancelled status; choosing a list per call; migrating lists from
`todov2-opencode` or from Claude Code's own task files.

Anti-goals [driver's call]: outcomes that make this a failure even if every tool works.

- Two agents in one repository hold different lists without being told.
- A change is lost, or the list file is left unreadable, by a crash or by two writers.
- Text one agent wrote reaches another agent's context without that agent calling a tool.
- A task disappears without trace. Whatever any tool call does, every task ever created is in
  the list or in the archive (8.5, 9.4). Rewriting a task's text is not this: update may change
  any field.
- The tracker disables, overrides or wraps a host's own list tool.

## 4. Constraints

- Repository pattern: one shared Rust engine under `shared/` and one thin wrapper per agent, as
  `shared/opencode-ctl` with `opencode-ctl-mcp-{cc,qwen,agy,opencode}`.
- `shared/*` crates are not release packages; their changes ship through the plugins that embed
  them, and each plugin releases on its own sequence
  (`docs/specs/2026-10-05-independent-plugin-versioning.md:19`, `:25`). Two agents can therefore
  run different engine versions against one list file. Section 8.4 answers that.
- Released zips carry binaries for Windows x64 and for Linux and macOS on x86_64 and aarch64 (root
  `README.md`, "Installing"). Every rule below must hold on all three systems.
- The context-cost gate applies: `tools/plugin-footprint` measures each plugin and
  `docs/footprints/budgets.json` holds its ceiling.
- Source files carry the repository's licence header (`lefthook.yml`, `license-headers`). The
  licence is PolyForm Noncommercial; this is source-available software, not open source.
- No network use, no background service: the engine runs only while an agent or a person runs it.

## 5. Approaches considered

Each fork was put to an independent peer (the live agy session) before the owner saw it. Every
claim that decided something was measured by the author; the measurements are in section 15.

### 5.1 The shape

| Option | What it is | Outcome |
|---|---|---|
| A | One shared Rust MCP engine, a thin wrapper per agent | Picked by both reviewers |
| B | A, but reading and writing Claude Code's own task files | Dropped: the format is private to another vendor and can change in any release; those tools are off by default on current models (section 15.3) |
| C | A command-line tool plus a skill, no server | Dropped: every call is a shell command behind permission prompts, and the model gets no typed tool |
| D | A file convention plus a skill, no code | Dropped: nothing protects two agents writing at once |
| E | A separate OpenCode-native build beside A (the peer's idea) | Not adopted: the lines and the API name the peer cited for it do not exist, and it means two implementations to keep in step |

### 5.2 Where the list lives

| | Folder in the working tree | Folder inside the git common directory | Per-user folder outside the repository |
|---|---|---|---|
| Shown by `git status --short` | no | no | no |
| Worktrees of one repository | each has its own list | share one list | share one list |
| `git clean -fdx` | deletes it | survives | survives |
| Carried by a clone | yes, once force-added | no | no |
| Repository folder renamed | follows | follows | orphaned (by construction, not measured) |
| Not a git repository | works | needs a fallback | works |

The author first leaned to the working tree. The peer argued for the per-user folder and its
objections to the working tree held when measured (deleted by `git clean -fdx`, split across
worktrees, able to arrive in a clone). The git common directory came out of those measurements;
neither reviewer had named it. Both then ranked it first. Picked: the git common directory, with
the per-user folder only where there is no git repository.

### 5.3 A list that cannot be written

The peer proposed falling back to the per-user folder when the git folder is not writable. The
author refused: an agent that can write and an agent that cannot would then keep two lists
without knowing. The peer agreed. Picked: fail the write loudly, keep reads working.

### 5.4 Removing completed tasks

1. The peer proposed a flag on create that clears completed tasks. Refused by the author with a
   traced case: on a shared list, a second session setting the flag removes the finished steps of
   the first session's plan. The peer agreed.
2. The author proposed archiving on the next create once every task is completed. The peer found
   the flaw: with one task created and completed, a create naming it as a blocker archives it
   first and then rejects the blocker as missing. The peer was right.
3. Picked: nothing is archived while a session runs. Archiving happens when an MCP server starts
   and finds every task completed, or when a person asks. A blocker that has left the list counts
   as satisfied.
4. The builder check on this document then found that deletion could erase another session's
   task, and that any server start, even a restart in the middle of a session, would empty a
   finished list at once. Picked by the author, after the owner approved the text: deletion
   applies only to an open task and keeps the record in the archive (9.4), and start-up archiving
   waits until no task has changed for 12 hours (8.5).

## 6. The approved text

The owner approved these fourteen points on 2026-11-18 ("approved, write the design doc"). The
peer signed each one. They are reproduced as approved; sections 7 to 13 carry them out and do not
replace them. Seven of them settle something the owner had not ruled on before approving the
text: points 2, 5, 8, 10, 11, 12 and 13.

1. **Shape.** One Rust engine crate under `shared/` implementing a stdio MCP server, plus four
   thin wrapper plugins (Claude Code, Qwen, Antigravity, OpenCode) that only start it. OpenCode
   reaches it through its MCP configuration. There is no second implementation of the tools in
   TypeScript.
2. **Where the list lives.** In a folder inside the repository's git common directory, found by
   walking up from the working directory and following a linked worktree's pointer to the shared
   directory, so all worktrees of one repository share it. In a directory that is not inside a
   git repository, the list lives in a per-user data directory keyed by that directory's
   canonical path.
3. **No silent fallback.** When the directory is inside a git repository but the folder cannot be
   created or written, the write fails with an error naming the path and the operating-system
   error, and reads still work. The engine never redirects that write to the per-user directory,
   because two agents would then hold two different lists without knowing.
4. **Which repository.** The engine takes the repository from its own process working directory
   on each call. An environment variable overrides it for a host that starts servers somewhere
   else. Every list reply states the resolved repository path on its first line, so a wrong
   resolution is visible. No tool has a path parameter for the model to fill in.
5. **One list per repository, one escape hatch.** There is one default list per repository. A
   second environment variable names a different list for a session that must be kept apart. No
   tool takes a list name, so the model cannot write to the wrong list by leaving an argument out.
6. **File and atomicity.** One JSON file per list holding every task, a next id that is never
   reused, and a format version. Every change takes an OS advisory lock on a sibling lock file,
   reads the file, applies the change, writes a temp file and renames it over the list. Only
   "blocked by" is stored; "blocks" is derived when listing, so a dependency cannot be half
   written.
7. **Tools.** Four tools: create, get, update, list. Create accepts one or more tasks in one
   call, each with a subject, an optional description, an optional owner, and optional blockers
   that name existing task ids or earlier tasks in the same call. Update changes one task: status
   (pending, in_progress, completed), subject, description, owner, adding and removing a blocker,
   and deleting the task. Deleting a task also removes its id from every other task's "blocked
   by" list under the same lock. Create and update reject any change that would create a
   dependency cycle. A blocker naming an id below the next id that is no longer in the list
   (archived or deleted) is accepted as already satisfied and not stored; a blocker naming an id
   that was never issued is rejected. List prints the resolved repository path on its first line,
   then one line per task without descriptions; get returns one task in full. No tool has a flag
   that removes completed tasks, and no call archives anything while a session is running.
   Completed tasks leave the list in two ways only: when the MCP server starts and finds every
   task in the list completed, it moves them all to an archive file beside the list (the reminder
   hook and the checklist subcommand also start the binary and never archive); and a person can
   archive completed tasks at any time with a subcommand of the engine binary. Archiving never
   reuses an id and never fails a call: if the archive cannot be written, the list is left as it
   is.
8. **A task left in progress by a dead session.** No lease, no process id, no automatic reset.
   Each task records which agent last changed it and when; list shows that; any agent takes a
   task over with an ordinary update.
9. **Reminder.** Each wrapper emits one line at session start, and again after a context
   compaction where the host signals one, stating how many tasks are open (pending or
   in_progress) in this repository's list and which tool to call. Claude Code and Qwen use
   SessionStart hooks; Antigravity uses a PreInvocation hook that acts only on the first
   invocation; OpenCode uses a one-shot flag in its context hook plus its compaction hook. The
   hook resolves the repository from the workspace path the host event supplies, falling back to
   the process working directory and the environment override. It carries no task text, prints
   nothing when no task is open, and does not run on every round.
10. **Host built-in lists.** The tracker does not disable, override or wrap any host's own list
    tool. Its tool descriptions say when to use it instead: for work that must outlive the
    session or be seen by another agent. `opencode/plugins/todov2-opencode` is not changed by
    this work; its overlap with OpenCode's built-in `todowrite` becomes its own ROADMAP entry.
11. **What a person sees.** The engine binary has a subcommand that prints the list as a readable
    checklist, and each wrapper exposes it as a command where the host has commands. No always-on
    panel or sidebar in the first release.
12. **Measure before planning.** Before the implementation plan is written, a throwaway spike
    measures in each of the four agents: the working directory an MCP server is started in,
    whether that process can write under `.git`, whether the host environment reaches it, and
    which session-start hook exists and what it receives. Any point above that a measurement
    contradicts is revised before the plan.
13. **Not in the first release.** Sharing a list between machines or through git; priorities; a
    cancelled status; choosing a list per call; migrating lists from `todov2-opencode` or from
    Claude Code's own task files.
14. **Context cost.** The plugin's resident cost (its tool schemas and any skill frontmatter) is
    measured by `tools/plugin-footprint` and held under a budget recorded in
    `docs/footprints/budgets.json`, as for every other plugin here. The design document sets the
    number; the aim is to stay below the resident figure of `rtk-mcp-cc` (3,690 bytes in the root
    README) [PROPOSED, UNMEASURED].

One clause of point 7 was added after the peer's last sign-off and the peer has not seen it: "the
MCP server starts" replaced "an engine process starts", with the parenthesis about the hook and
the subcommand. The owner approved the text with that clause in it.

Sections 7 to 13 narrow or extend the approved text in four places. Those were written after the
approval, in answer to the reviews of this document, and are listed first in section 17.3.

## 7. Finding the list

### 7.1 The starting directory

Every entry point resolves a starting directory D, then a list from D (7.2 to 7.5).

| Entry point | D is the first of these that is present |
|---|---|
| MCP server, on each tool call | `REPO_TASKS_DIR`; the client's first root (below); the process working directory, except on Antigravity (below) |
| Reminder (section 10) | `REPO_TASKS_DIR`; the workspace path in the host's event, which on OpenCode arrives as the `--workspace` argument (10.2); the process working directory, except on Antigravity (10.2) |
| A subcommand run by a person (section 11) | the `--dir` argument; `REPO_TASKS_DIR`; the process working directory |

`REPO_TASKS_DIR` counts as present when it is set and not blank. It must be an absolute path to an
existing directory; otherwise every call fails with an error naming the variable and its value
(the reminder prints nothing instead, 10.1). The server reads the working directory on each call,
not once at start [approved, point 4]. The reminder and the subcommands are separate processes with
no client, so the root does not enter their rows.

**The client's first root** [owner ruling, 2026-11-18, after the spike: "Yes to both, revise 7.1 and
8.5"; it extends point 4 of the approved text, which names only the process working directory and
the variable]. The spike (15.7) found that Antigravity starts an installed server in the plugin's
own folder, with no variable naming the workspace, while all four hosts declare the MCP `roots`
capability and answer `roots/list` with the starting directory. So:

- When the client declared `roots` in `initialize`, the server sends `roots/list` at once when it
  receives `notifications/initialized`, and again after each `notifications/roots/list_changed`.
  A request is **settled** when its reply arrives or 2 seconds [PROPOSED, UNMEASURED] after it was
  sent, whichever is first. A reply to the first request that arrives later is used only if no
  tool call has yet resolved D; after one has, it is ignored, because adopting it would move later
  calls to another list than the earlier ones used. A reply to a refresh (below) is always used.
  Only one request is in flight at a time: a `roots/list_changed` notification that arrives
  meanwhile is remembered, and one more request is sent when the request settles, by its reply or by the 2
  seconds running out. A client that
  declared no `roots` has nothing to settle. The 2 seconds start when the request is sent, so a
  tool call waits only for what is left of them, and once a request is settled no later call
  waits for it again.
- The reply's roots are read in order; a root's `uri` is converted to a local path first, and the
  first whose path is an existing directory becomes the root directory. A `file://` URI is read
  as RFC 8089 gives it [CLAIM, 2026-11-18]: the
  authority is empty or `localhost` (any other authority, such as a network share, makes the root
  unusable), the path is percent-decoded as UTF-8 (a byte sequence that is not valid UTF-8 makes
  it unusable), and on Windows a path `/L:/x` with any ASCII letter L, upper or lower case, becomes `L:\x`. Any other scheme is
  unusable. Other roots are ignored: the spike saw one root per host and never several [driver's
  call, unmeasured].
- A tool call that arrives while a request is not settled waits for it, as above, then goes on.
  A tool call that arrives before `notifications/initialized` is refused with an error saying the
  server is not initialized (a JSON-RPC error response with code -32600, not a tool result: it is
  a protocol violation, not a refused task [driver's call]), and touches no list: the protocol puts that notification before any
  such call [CLAIM, 2026-11-18], and answering it from the working directory could open the
  wrong list.
- A client that declared no `roots`, a failed or empty reply, a request that was settled by the
  2 seconds running out, and a reply with no usable root all mean "no root": D falls to the process working directory, except on Antigravity. A refresh
  that fails, times out or has no usable root keeps the root already held.
- **Antigravity has no working-directory fallback for the server**, as it has none for the
  reminder (10.2): its installed server starts in the plugin's folder (15.7), so with neither the
  variable nor a root every call fails with an error that names `REPO_TASKS_DIR` and says the
  host gave no workspace. The wrapper's compiled-in host (8.1) decides this.
- The root is held as the last answer received and used by every later call; D is still resolved
  on each call, so a changed root is followed. A call resolves D once, when it starts, and uses
  that D until it ends; a reply that arrives during a call changes the root for later calls only.
- Tests must cover each of these, in both directions: a late first reply is accepted before any
  call has resolved D and ignored after one has; a notification during a request yields exactly
  one follow-up request, sent after a reply and after a timeout alike; no second request is ever
  in flight; a call before `initialized` is refused and touches no list; a client that declared
  no `roots` is never sent a request.
- The 2 seconds are a constant of the engine, not a setting; the engine's tests may shorten it
  through a parameter that no host configuration reaches, so that a test with a client that never
  replies does not take 2 seconds.

The variable comes first for the reminder as well [driver's call]. Point 9 names the event's path
first; but point 4 makes the variable an override, and a reminder that counted one list while the
tools served another would mislead. For the same reason `reminder` has no `--dir` argument: an
argument that outranked the variable would bring the mismatch back. `--workspace` is only the
form in which a host's event path arrives when the host passes no event on standard input.

### 7.2 The git common directory

Walk up from D to the filesystem root. At the first directory that has an entry named `.git`:

- if `.git` is a directory, that directory is the git directory G;
- if `.git` is a file whose content starts with `gitdir: `, G is the path after that prefix,
  with trailing whitespace and line endings trimmed, resolved against the directory holding the
  file.

G must be an existing directory that contains a file named `HEAD`. A `.git` entry that does not
lead to one (a worktree whose main repository is gone, an unreadable file) makes every call fail
with an error naming the entry. It is never treated as "not a git repository", because that would
silently open a different list [driver's call].

The git common directory C is the content of the file `G/commondir` when that file exists
(trimmed the same way, and resolved against G when it is relative), and G itself otherwise. C is
then made canonical and absolute. On Windows the `\\?\` prefix that canonicalization adds is
removed wherever a path is shown or hashed.

The oracle is `git rev-parse --path-format=absolute --git-common-dir` run in D. The engine's
answer must equal the oracle's in each of these cases, and tests must cover each:

1. D is the top of a repository.
2. D is a subdirectory of a repository.
3. D is the top of a linked worktree, and a subdirectory of one.
4. D is inside a submodule (the oracle answers a path under the superproject's `.git/modules/`;
   a submodule therefore has its own list).

The engine does not run `git` [driver's call]: a server whose environment lacks `git` on its
`PATH` would otherwise fall to the non-git case while another agent's server found the
repository, and the two would hold different lists.

Deliberate differences from the oracle [driver's call]: the environment variables `GIT_DIR`,
`GIT_COMMON_DIR` and `GIT_CEILING_DIRECTORIES` are ignored, and a bare repository (no `.git`
entry) is treated as "not a git repository". The oracle comparisons run with those variables
unset.

### 7.3 The store folder and its files

Inside a git repository the store folder is `C/repo-tasks/`. It holds, per list name N:

| File | What it is |
|---|---|
| `N.json` | the list (section 8.1) |
| `N.lock` | the lock file; its content is never read (section 8.2) |
| `N.json.tmp` | a temp file that exists only during a write (section 8.2) |
| `N.archive.jsonl` | the archive (section 8.5) |

The folder is created on the first write. On Unix it is created with mode 0700 and its files with
mode 0600 [driver's call]. A missing folder or a missing `N.json` reads as an empty list and
creates nothing.

### 7.4 The list name

N is `REPO_TASKS_LIST` when that variable is set and not blank, and `default` otherwise. N must
match `[A-Za-z0-9_-]{1,64}`. A value that does not match fails every call with an error naming
the variable; it is never altered to fit [driver's call].

### 7.5 Outside a git repository

When the walk in 7.2 finds no `.git` entry, the store folder is `B/repo-tasks/K/` where:

- B is the per-user data directory [driver's call]: on Windows `%LOCALAPPDATA%`; on macOS
  `$HOME/Library/Application Support`; elsewhere `$XDG_DATA_HOME` when set and not blank, else
  `$HOME/.local/share`.
- K is the first 16 hexadecimal characters of the SHA-256 of the UTF-8 bytes of canonical D. On
  Windows the `\\?\` prefix that canonicalization adds is removed before hashing. No case
  folding is applied; two spellings of one directory give one K only if canonicalization returns
  the spelling stored on disk [CLAIM, 2026-11-18, not measured; added to 15.6].

The folder also holds `directory.txt` containing canonical D, so a person can tell the folders
apart. Known limit: starting from a subdirectory of a non-git directory gives a different K and
therefore a different list.

### 7.6 A store that cannot be written [approved, point 3]

If the store folder cannot be created, or a file in it cannot be written, the call that needed the
write fails with an error that names the path and the operating-system error. `task_list` and
`task_get` still answer from whatever can be read. The engine never writes a list anywhere other
than the folder 7.3 or 7.5 gives.

## 8. The list file

### 8.1 Shape

`N.json` is one UTF-8 JSON object:

```json
{
  "format": 1,
  "next_id": 4,
  "tasks": [
    {
      "id": 1,
      "subject": "Reproduce the failing upload",
      "description": "Run the upload test against a 2 GB file and keep the log.",
      "status": "completed",
      "owner": "ck",
      "blocked_by": [],
      "created_at": "2026-11-18T10:12:00Z",
      "updated_at": "2026-11-18T10:40:31Z",
      "updated_by": "claude-code"
    }
  ]
}
```

Rules:

- `format` is the integer 1.
- `next_id` is an integer greater than every id ever issued in this list. It never decreases. The
  first task of a new list gets id 1.
- `tasks` is ordered by `id` ascending. `id` is a positive integer, unique in the list.
- `subject` is required, 1 to 200 characters after trimming. `description` and `owner` are
  optional and are omitted, never null, when absent. `description` is at most 8,000 characters;
  `owner` is at most 64. Lengths count Unicode scalar values. The three limits are
  [PROPOSED, UNMEASURED].
- `subject` and `owner` contain no control character (U+0000 to U+001F, U+007F), so each stays on
  one line and cannot carry terminal escape sequences. `description` may contain line feeds and
  tabs and no other control character [driver's call].
- `status` is one of `pending`, `in_progress`, `completed`.
- `blocked_by` is an array of ids, ascending, without duplicates, never containing the task's own
  id, and containing only ids of tasks present in the list. It may name completed tasks.
- `created_at` and `updated_at` are strings in RFC 3339 form, UTC, whole seconds, ending in `Z`.
  They are strings, never numbers. The only rule that depends on a timestamp is the age test in
  8.5.
- `updated_by` names who made the last change: `claude-code`, `qwen`, `antigravity`, `opencode`
  (the wrapper's compiled-in host, narrowed the way `shared/opencode-ctl/src/host.rs` narrows it
  for OpenCode), or `cli` for a subcommand run by a person.

### 8.2 Writing [approved, point 6]

Every change follows the same five steps:

1. Open `N.lock`, creating it if needed, and take an exclusive OS advisory lock on it, repeating a
   non-blocking attempt until it succeeds or 5 seconds [PROPOSED, UNMEASURED] have passed.
   `shared/opencode-ctl/src/pairing.rs:427` takes the same kind of lock in its blocking form;
   this design needs the form with a deadline. The operating system releases the lock when the
   process dies. A writer that does not get the lock fails the call; it never proceeds without it.
2. Read `N.json`. A missing file is the empty list: format 1, next id 1, no tasks.
3. Check and apply the whole change in memory. If any rule in sections 8 or 9 is broken, fail the
   call and leave the file untouched. A call changes everything it asked for or nothing.
4. Remove `N.json.tmp` if a crash left one behind (holding the lock makes that safe). Write the
   new content to `N.json.tmp`, flush it to disk, rename it over `N.json`
   (`pairing.rs:282` and `:329` show the pattern), and on Unix flush the folder to disk as well,
   so that a power cut leaves the old list or the new one and never neither. If the write or the
   rename fails, the call fails, the old list stands, and the temp file is removed. One case is
   retried first: on Windows a rename over a file that a reader holds open at that moment can be
   refused [CLAIM, 2026-11-18, the author's knowledge of Windows; not measured, 15.6]. A rename
   refused for that reason is repeated for up to 5 seconds [PROPOSED, UNMEASURED], counted from
   the first refusal and not from step 1, so that the time the lock took does not change it. Only
   an error of kind `PermissionDenied` or a sharing violation is repeated, because Windows may
   report this case as either [CLAIM, 2026-11-18, not measured]; every other error fails the call
   at once. The Windows test of 8.3 records which kind appears. `pairing.rs:329` does not retry.
5. Release the lock.

### 8.3 Reading

`task_list`, `task_get`, the reminder and the checklist subcommand take no lock. The rename in
step 4 means a reader sees the whole old file or the whole new file. A reader opens the file,
reads all of it and closes it at once; on Windows it opens the file so that it may be replaced
while open (Rust's standard library does this by default), and a read refused because the file is
being replaced at that moment (the same two error kinds as in 8.2) is repeated for up to one
second [PROPOSED, UNMEASURED]. The tests
that run on CI's Windows runner (15.2) include one with a writer and several readers on one list,
in which no call may fail.

A reader that needs both files reads the list first and the archive second. A task is appended to
the archive and flushed before the list is rewritten without it (8.5), so a task missing from the
list that was read already has a complete line in the archive.

A file that is not valid JSON, or breaks a rule in 8.1, is never repaired or overwritten
[driver's call]. Every call fails with an error naming the file, except the reminder, which
prints the notice of 10.1. Naming the file lets a person repair or remove it by hand; the first
release has no repair command, so the tools stay unusable for that list until a person acts.

### 8.4 Two engine versions on one file [driver's call]

Plugins release independently (section 4), so an older and a newer engine can share a list.

- An engine that reads `format` greater than the newest it knows fails every call, reads
  included, with an error that names both numbers and says to update the plugin. It does not
  guess at a newer layout.
- Members an engine does not know, at the top level or inside a task, are kept unchanged when it
  rewrites the file.
- A later engine may add an optional member without changing `format` only when an engine that
  ignores the member still behaves correctly. Any other change increments `format`.
- The lock file's name, the locking call and the five steps of 8.2 are part of format 1.

Only format 1 exists; the first release contains no migration code.

### 8.5 The archive [approved, point 7]

`N.archive.jsonl` holds one JSON object per line: a task exactly as it stood in the list, plus
exactly one of `"archived_at"` or `"deleted_at"`, in the form of 8.1. It is only ever appended to.

Moving a set of tasks to the archive, under the lock of 8.2:

1. Append their lines to the archive and flush it to disk. When the file is not empty and its
   last byte is not a line feed (a crash left half a line), the append begins with a line feed,
   so a new line never joins an old fragment.
2. Rewrite the list without them (8.2, step 4), removing their ids from every remaining task's
   `blocked_by`.

A crash between the two steps, or a failure of step 2, leaves a task in both files; a later move
appends it again. The list wins: every reader of the archive ignores a line whose id is in the
list, uses the last line for any other id, and ignores any line that is not a complete JSON object,
wherever it stands in the file. A damaged archive line never fails a call and is never repaired
(8.3 governs the list file only). The archive is never cut back to undo step 1. `next_id` is unchanged by any move.

A task reaches the archive in exactly three ways:

- **MCP server start.** After the `initialized` notification, once the question of a root is
  answered (the client declared no roots, or its reply to `roots/list` arrived, usable or not),
  and before `serve` answers its first tool call, it resolves the list as a tool call would and reads
  it without the lock. A host that never sends `initialized`, and a request
  that is only settled by the 2 seconds of 7.1 running out, never trigger it: an unanswered root
  must not let it act on the working directory's list, and the next start tries again. It is
  decided once, when the question is first settled; a reply that arrives afterwards does not
  re-arm it. The condition is: the list holds at least one task,
  every task is completed, and every task's `updated_at` is at least 12 hours in the past
  [PROPOSED, UNMEASURED]. Only when the condition holds does it make one non-blocking attempt at
  the lock, read the list again under it, and, if the condition still holds, archive them all
  with `archived_at`. In every other case, and on any failure, nothing is written and the server
  starts normally; a failure writes one line to standard error. A repository with no list is not
  touched: no folder and no lock file are created.
- **A person.** The `archive` subcommand (section 11) archives every completed task, whatever its
  age, with `archived_at`.
- **Deletion.** `task_update` with `delete: true` moves one open task to the archive with
  `deleted_at` (9.4).

No tool call moves a completed task anywhere, and the reminder and the other subcommands move
nothing.

The 12-hour condition is the author's addition to point 7 [driver's call]. Without it, a host
that restarts its server in the middle of a session, or a second agent opened a moment after a
plan is finished, would empty a list the person is still looking at. The test uses the machine's
clock; a clock set backwards only delays archiving.

The engine never trims the archive. A person may delete the file; `task_get` can then no longer
answer for the tasks it held.

## 9. The tools

The server exposes four tools. Names and parameters are a contract.

### 9.1 Rules common to all four

- An id is a JSON integer. A string, even one made of digits, is rejected.
- Every reply is a single block of plain text.
- A failed call returns an MCP tool error whose text is one line saying what was refused and why,
  and it changes nothing (8.2, step 3).
- This rule is about the id a call acts on, the `id` of `task_get` and `task_update`. Ids named as
  blockers follow 9.6 instead. When that id is not in the list, the error says which case
  applies: the id was never issued (it is below 1, or at or above `next_id`); or the task has
  left the list (`task_update`; `task_get` answers from the archive instead, 9.3); or, for
  `task_get` only, the task has left the list and the archive holds no record of it.

### 9.2 `task_create`

```json
{
  "tasks": [
    {
      "subject": "string, required",
      "description": "string, optional",
      "owner": "string, optional",
      "blocked_by": [1, 2],
      "blocked_by_earlier": [1]
    }
  ]
}
```

- `tasks` holds 1 to 50 entries [PROPOSED, UNMEASURED]. They receive consecutive ids in order.
- `blocked_by` names ids already issued (9.6). `blocked_by_earlier` names earlier entries of this
  same call by 1-based position, and each value must be lower than the entry's own position
  [driver's call on the encoding].
- New tasks start as `pending`.
- Reply: one line per task, `created #<id> <subject>`, followed by one line for each blocker that
  was accepted as already satisfied (9.6):
  `#<id>: blocker #<blocker id> has left the list and counts as satisfied`, where `<id>` is the
  task that named it. All `created` lines come first, in id order; the satisfied lines follow,
  ordered by task id and then by blocker id.

### 9.3 `task_get`

`{ "id": 3 }`. Reply, in this order:

```text
id: 3
subject: Add a regression test
status: pending
owner: ck
blocked_by: #2, #4
blocks: #5
created_at: 2026-11-18T10:12:00Z
updated_at: 2026-11-18T10:40:31Z
updated_by: claude-code

The description, exactly as stored.
```

`blocked_by` lists every stored blocker, completed ones included. `blocks` is derived: the ids of
tasks in the list whose `blocked_by` contains this id. The `owner`, `blocked_by` and `blocks`
lines are left out when there is nothing to show, and so are the blank line and the description
when the task has none.

A task that has left the list is answered from the archive [driver's call], read as 8.3 and 8.5
say: the same lines, preceded by one line reading `archived <archived_at>` or
`deleted <deleted_at>`, and without a `blocks` line. The call fails when the id was never issued,
or when the archive holds no line for it (9.1).

### 9.4 `task_update`

```json
{
  "id": 3,
  "status": "in_progress",
  "subject": "string",
  "description": "string",
  "owner": "string",
  "add_blocked_by": [2],
  "remove_blocked_by": [1],
  "delete": true
}
```

- Only `id` is required; at least one other member must be present. `delete: false` counts as
  absent.
- A call that would change nothing (for example the status it already has, or a blocker already
  stored) still succeeds, writes nothing, and leaves `updated_at` and `updated_by` as they were.
  A call that changes anything sets both. The test of 8.5 therefore measures real changes, and a
  repeated no-op cannot hold off archiving [driver's call].
- `remove_blocked_by` follows 9.6 for a blocker's number: an id below 1, or at or above
  `next_id`, fails the call; any other id that is not among the task's stored blockers is
  ignored. An id named in both `add_blocked_by` and `remove_blocked_by` fails the call.
- The id must be in the list; otherwise the call fails (9.1).
- An empty `owner` or `description` removes that member.
- Any status may follow any status. The engine does not stop a task starting or completing while
  a blocker is open: `blocked_by` is information for the reader, not a gate [driver's call].
- Any agent may update any task [approved, point 8].
- `delete: true` is for a task that will not be done. It is allowed only on a task that is
  pending or in_progress, and it cannot be combined with any other change. The task moves to the
  archive with `deleted_at` (8.5) and its id is removed from every other task's `blocked_by`. If
  the archive cannot be written, the call fails and the task stays.
- `delete: true` on a completed task fails: completed tasks leave the list only by archiving
  [approved, point 7].
- Reply: `updated #<id>` or `deleted #<id>`, plus a line per blocker accepted as satisfied, in
  the form 9.2 gives.
- Known limit: a completed task can be set back to pending and then deleted, in two calls. That
  is not prevented, because reopening a task is ordinary work and a reopened task is an open
  task. The rule above stops a single call removing finished work; the record survives either
  way, in the archive with `deleted_at`.

How deletion is carried out is the author's reading of point 7 [driver's call]. Point 7 lists
"deleting the task" among the changes update can make, and also says completed tasks leave the
list in two ways only. Read together, deletion cannot apply to a completed task. Keeping the
deleted record in the archive, rather than erasing it, is what keeps the anti-goal in section 3:
no task disappears without trace.

### 9.5 `task_list`

No parameters. Reply:

```text
repo-tasks: /home/ck/src/widget/.git (list default, from /home/ck/src/widget-fix/src)
#1 [x] Reproduce the failing upload [owner ck]
#2 [>] Fix the chunk size [claude-code 2026-11-18T10:41:07Z]
#3 [ ] Add a regression test [blocked by #2]
```

- Line 1 inside a git repository is `repo-tasks: <C> (list <N>, from <D>)`: the git common
  directory that identifies the list, the list name, and the starting directory the call resolved
  from. For a linked worktree C is the main repository's git directory, which is exactly what
  shows that the list is shared [driver's call on reading point 4's "resolved repository path"].
  Outside a git repository line 1 is `repo-tasks: <D> (list <N>, not a git repository)`.
- Then one line for every task in the list, in id order: `#<id>`, a mark (`[ ]` pending, `[>]`
  in_progress, `[x]` completed), the subject, then in this order when they apply:
  `[owner <owner>]`; `[blocked by #a, #b]` listing only blockers that are not completed; and for
  an in_progress task `[<updated_by> <updated_at>]`, which is how a task left by a dead session
  shows.
- The reply is never shortened. It grows with the list until tasks are archived (17.2, item 3).
- An empty list prints line 1 and then `no tasks`. When an archive file exists it adds the line
  `archived tasks exist: the "list --archived" subcommand prints them`.

### 9.6 Blockers and cycles [approved, point 7]

For each id named in `blocked_by` or `add_blocked_by`:

- the id is in the list: it is stored;
- the id is at least 1, below `next_id`, and not in the list: it is accepted as already
  satisfied, not stored, and the reply says so;
- the id is below 1, or at or above `next_id`: the call fails.

After a change is applied in memory, the `blocked_by` relation over the tasks in the list must
contain no cycle, and no task may block itself; otherwise the call fails.

## 10. The reminder [approved, point 9]

### 10.1 What it says

Exactly one line, with n the number of tasks that are pending or in_progress and m the number
that are in_progress:

```text
repo-tasks: <n> open in this repository's task list, <m> in progress. Call the task_list tool to see them.
```

It contains no subject, description or owner. When n is 0, when there is no list, or when
anything else fails, it prints nothing and still exits successfully: a reminder never blocks or fails
a session.

One failure is not silent [driver's call]. When the list file exists and cannot be used (it is
not valid JSON, breaks a rule in 8.1, has a newer `format`, 8.3 and 8.4, or cannot be opened or
read), or when 7.2 stops with an error on a `.git` entry that cannot be followed, every tool call
will fail, and an agent told nothing would not know a list exists. The reminder then prints this line
instead, and still exits successfully:

```text
repo-tasks: this repository's task list cannot be read. Call the task_list tool to see why.
```

The line is produced by the engine binary's `reminder` subcommand and by nothing else. No wrapper
reads the list file itself.

### 10.2 Per host

| Host | When it runs | How the line reaches the model | Event field that carries the workspace path |
|---|---|---|---|
| Claude Code | `SessionStart` for the sources `startup`, `resume`, `clear`, and the compaction source | `hookSpecificOutput.additionalContext`, as `claude-code/plugins/re-ghidra-mcp-cc/src/bin/hook.rs:78-80` | `cwd`, then the variable `CLAUDE_PROJECT_DIR`, as that file's lines 41-43 |
| Qwen | `SessionStart` for `startup`, `resume`, `clear` (`qwen/plugins/re-ghidra-mcp-qwen/qwen-extension.json:17-43`) | `systemMessage` and `hookSpecificOutput.additionalContext`, as `qwen/plugins/re-ghidra-mcp-qwen/src/bin/hook.rs:77-80` | the path that hook reads from its event |
| Antigravity | `PreInvocation`, acting only when `invocationNum` is at most 1 (the check in `antigravity/plugins/re-ghidra-mcp-agy/src/bin/hook.rs`) | `injectSteps`, as that file's line 78 | the first entry of `workspacePaths`, as `antigravity/plugins/re-ghidra-mcp-agy/src/bin/hook.rs:89-96` reads it, else `cwd`, both on standard input |
| OpenCode | on the first `context` event of each session id (the event carries `sessionID`, as `opencode/plugins/todov2-opencode/plugin.ts:18` reads it), and again from the `compaction` hook (the `Plugin.define` block of `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts`) | pushed onto `event.system` as `{ type: "text", text: <the line> }`, and only when `event.system` is an array, as `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts:203-204` does | `ctx.location.directory`, passed to the binary as `--workspace` |

D then follows the order in 7.1. Host rules [driver's call]:

- **One binary per wrapper.** On Claude Code, Qwen and Antigravity the hook's command is the
  wrapper's own binary run as `reminder --event <host>`. There is no separate hook binary.
- **Antigravity never falls back to the process working directory.** Its documentation says a
  hook's working directory is the folder holding `hooks.json`, which is the plugin, not the
  workspace. With neither `REPO_TASKS_DIR` nor a workspace path in the event, the reminder prints nothing.
- **OpenCode's `plugin.ts` runs the binary** as `reminder --event opencode --workspace <path>` (as `opencode/plugins/rtk-mcp-opencode/plugin.ts`
  runs its own with `spawnSync`) and pushes whatever line comes back, in the object form the
  table gives; when nothing comes back it pushes nothing. It reminds once per session id, not
  once per plugin load, and it contains no code that parses the list.

The name of Claude Code's compaction source (`compact` [CLAIM, 2026-11-18, from the author's
memory of the documentation]) and whether Qwen and Antigravity signal a compaction at all are for
the spike (section 16). Where a host signals none, the reminder runs at session start only.

## 11. The command line [approved, points 7 and 11]

Each wrapper ships one binary that contains the engine. Its subcommands:

| Subcommand | Does |
|---|---|
| `serve` | runs the MCP server on standard input and output (OpenCode adds `--host opencode`, as `opencode-ctl` does) |
| `reminder --event <host>` | `<host>` is `claude-code`, `qwen`, `antigravity` or `opencode`; prints the line of 10.1 wrapped the way that host expects (10.2), the bare line for `opencode`, or nothing |
| `list` | prints exactly what `task_list` replies; `list --archived` prints the archive (below) |
| `archive` | archives every completed task (8.5) and prints one line, `archived <n> tasks` (`archived 1 task` for one), or `no completed tasks` when there are none |
| `--version` | prints the version |

`list` and `archive` accept `--dir <path>`; `reminder` accepts `--workspace <path>` and no `--dir`
(7.1). No subcommand moves a task back from the archive in the first release.

`list --archived` prints line 1 exactly as `list` does, then one line per archived task in id
order, read as 8.5 says (one line per id, and none for an id still in the list). Each line has
the form of 9.5 with the mark the task had when it left, without the `[blocked by ...]` and
in_progress parts, and ends with `[archived <archived_at>]` or `[deleted <deleted_at>]`. With no
archive file, or none of its lines left to show, it prints line 1 and `no archived tasks`.

Each wrapper also ships a command that runs `list`, on every host that has commands. The command
is named `repo-tasks`, so that it cannot collide with a host's own task or todo commands
[driver's call]. This
repository has command folders for OpenCode and Qwen today
(`opencode/plugins/aiplugins-devtools-opencode/commands`, `qwen/plugins/example/commands`); the
form for Claude Code and whether Antigravity has one are the builder's to settle.

## 12. Hosts' own lists, and trust [approved, point 10]

The tracker adds four tools and removes or changes nothing a host provides. Each tool description
carries, once, the sentence that tells a model when to choose it: use these tools for work that
must outlive the session or be seen by another agent.

`opencode/plugins/todov2-opencode` is not touched. OpenCode 2.3.0 ships a built-in `todowrite`
again as an opt-in (section 15.4), under the same tool name the plugin registers. The first pull
request of the implementation adds a ROADMAP entry recording that overlap for the owner to rule
on.

Trust: the list is as trusted as the repository's other local files. Anyone who can write the git
directory can write it, and a reply presents its content as task data that some agent or person
wrote. The first release adds no signing and no per-agent permissions; the reminder's lack of
task text (10.1) is the one place where text is deliberately kept out of a context.

## 13. Context cost [approved, point 14]

- The measured quantity is `tiers.resident.bytes` in `docs/footprints/repo-tasks-mcp-cc.json`
  (and the matching file for each other wrapper), produced by `tools/plugin-footprint`.
- Design target: at most 3,000 bytes resident per wrapper [PROPOSED, UNMEASURED].
- Stop line: the approved aim is to stay below `rtk-mcp-cc`'s measured 3,690 bytes
  (`docs/footprints/rtk-mcp-cc.json`). A wrapper that measures 3,690 bytes or more is reported to
  the owner before any budget is recorded for it.
- The entry in `docs/footprints/budgets.json` follows the existing ones: `deltaBytes` 500,
  `headroomBytes` 2000, and `residentBytes` set to the measured figure plus the headroom. That
  ceiling is not the same quantity as the measured figure: `rtk-mcp-cc` measures 3,690 and has a
  ceiling of 5,696.
- The first release ships no skill, so no skill frontmatter adds to the resident figure.
- The implementation plan includes a check that fails when a wrapper's measured resident figure
  is 3,690 bytes or more, so that the stop line does not depend on anyone remembering it.

## 14. Interfaces

Everything in this table is a contract: other processes, other engine versions or people depend
on it.

| Thing | Name |
|---|---|
| Engine crate | `shared/repo-tasks` |
| Wrapper plugins | `claude-code/plugins/repo-tasks-mcp-cc`, `qwen/plugins/repo-tasks-mcp-qwen`, `antigravity/plugins/repo-tasks-mcp-agy`, `opencode/plugins/repo-tasks-mcp-opencode` |
| Binaries | `repo-tasks-cc-mcp`, `repo-tasks-qwen-mcp`, `repo-tasks-agy-mcp`. The OpenCode wrapper installs the Claude Code wrapper's binary and passes `--host opencode`, as `opencode-ctl-mcp-opencode` does (`opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:91`, `:132`) |
| MCP tools | `task_create`, `task_get`, `task_update`, `task_list` (section 9) |
| Environment variables | `REPO_TASKS_DIR` (7.1), `REPO_TASKS_LIST` (7.4) |
| Store folder | `<git common directory>/repo-tasks/` (7.3); outside git `<per-user data directory>/repo-tasks/<key>/` (7.5) |
| Files | `<list>.json`, `<list>.lock`, `<list>.json.tmp`, `<list>.archive.jsonl`, and outside git `directory.txt` |
| List file | section 8.1, format 1 |
| Archive file | section 8.5 |
| Reminder line | section 10.1 |
| Subcommands | `serve`, `reminder`, `list`, `archive`, `--version` (section 11) |

All names in this table follow from the working name `repo-tasks` and change with it (17.2).

## 15. Measured, and not measured

### 15.1 Git behaviour (a throwaway repository, git 2.x, Linux, 2026-11-18)

- A folder inside the git common directory: `git status --short` prints nothing with it present;
  `git rev-parse --path-format=absolute --git-common-dir` printed the same path from the main
  checkout and from a second worktree; a clone does not contain it; it is still there after
  `git clean -fdx`, `git gc --prune=now`, `git worktree remove`, `git worktree prune`, and after
  the repository folder is renamed.
- A self-ignoring folder in the working tree: `git status --short` prints nothing; a second
  worktree does not see it; after `git add -f` and a commit, a clone contains it;
  `git clean -fdx` removes it.

### 15.2 This repository (at commit `23bb904`)

- Lock, temp file and rename: `shared/opencode-ctl/src/pairing.rs:282`, `:329`, `:427`.
- The existing engine learns its project from the working directory:
  `shared/opencode-ctl/src/engine.rs:46`, `cli.rs:248`.
- Session-start patterns exist for all four hosts in the `re-ghidra-mcp-*` plugins (the files
  named in 10.2).
- `opencode/plugins/todov2-opencode` keeps its list in OpenCode's plugin storage, which nothing
  outside OpenCode can read.
- OpenCode's installed MCP command is an absolute path
  (`opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:132`, and `:148` on why).
- CI runs the workspace's tests on Ubuntu, Windows and macOS (`.github/workflows/ci.yml:55-62`),
  so the engine's own tests run on all three.

### 15.3 Claude Code 2.1.327 (read from the installed binary's strings)

- The tools `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate` and `TodoWrite` are present.
- They are switched on for a fixed set of model ids ending at `claude-opus-4-7`,
  `claude-sonnet-4-6` and `claude-haiku-4-5`, and otherwise only when
  `CLAUDE_CODE_ENABLE_TODO_TOOLS` is true; `CLAUDE_CODE_ENABLE_TASKS` set to false turns Tasks
  off. In the owner's session on `claude-fable-5-2` none of them was offered.
- Its own store is one JSON file per task under `<config>/tasks/<list id>/` with a high-water
  mark file and a lock; the list id is `CLAUDE_CODE_TASK_LIST_ID` or derived from the session. A
  task there has id, subject, description, activeForm, owner, status (pending, in_progress,
  completed), blocks, blockedBy and metadata.

### 15.4 OpenCode 2.3.0 (source read at the installed tag, commit `a6b15d3`; not run)

- A local MCP server's working directory is its configured `cwd` resolved against the workspace
  directory, or the workspace directory when none is configured
  (`packages/core/src/mcp/stdio.ts:43-44`, used at `packages/core/src/mcp/client.ts:470`).
- The host environment is merged into a local server's environment
  (`packages/core/src/mcp/stdio.ts:30-33`).
- A built-in `todowrite` exists (`packages/core/src/tool/plugin/todo.ts`), opt-in through
  `{ "experimental": { "todo": true } }` (`CHANGELOG.md:1244`).

### 15.5 Antigravity (its bundled documentation, under `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/`)

- A hook command's working directory is the folder holding `hooks.json` (`hooks.md:129-131`); the
  workspace arrives as `"cwd"` on standard input (`hooks.md:165`) and as `"workspacePaths"`
  (`hooks.md:159`).
- `SessionStart` and `PreInvocation` events exist. No restriction on where a started process may
  write is stated.

### 15.6 Not measured

- Measured on Linux in the spike (15.7): the working directory given to an MCP server, whether it
  may write under `.git`, and whether the host environment reaches the server and a hook, for
  Claude Code, Antigravity and OpenCode; Claude Code's compaction signal. Still not measured:
  a compaction in Qwen (needs a model call), OpenCode through its background service, a compaction
  in Antigravity and in OpenCode.
- Any hook event of Qwen beyond those this repository already uses.
- How the four hosts behave on Windows and macOS. Every measurement in 15.1 ran on one Linux
  machine.
- Whether `CLAUDE_CODE_ENABLE_TODO_TOOLS` switches Claude Code's own tools back on.
- Whether a rename over a file a reader holds open is refused on Windows, and how often (8.2,
  8.3). This machine cannot measure it; the CI test named in 8.3 is the measurement.
- Whether Windows canonicalization returns one spelling for two differently cased starts of the
  same directory (7.5); a test on CI's Windows runner decides it.
- Every figure marked [PROPOSED, UNMEASURED].

### 15.7 Spike results (2026-11-18, Linux, a throwaway repository and a linked worktree)

Each host was started from three places: the repository root `R`, `R/sub/deep`, and a linked
worktree `W`. The probe was a stdio MCP server and a hook script that wrote one JSON line per
start; the host's environment held `REPO_TASKS_DIR=/spike/dir` and `REPO_TASKS_LIST=spike-list`.

| Cell | Claude Code 2.1.327 | Qwen Code 0.28.0 | Antigravity (agy 1.5.11) | OpenCode 2.3.0 (`--standalone`) |
|---|---|---|---|---|
| Server working directory | the starting directory, in all three places | the starting directory, in all three places (`QWEN_HOME` isolated; the model was never reached) | the plugin folder, in all three places: `<root>/.agents/plugins/probe` for a plugin inside the repository, `~/.gemini/config/plugins/probe` for an installed one (second test below); the inherited variable `PWD` was the starting directory only while the launching shell's `PWD` was | the starting directory, in all three places |
| May the server create a folder in the git common directory | yes; `W` resolved to `R/.git` | yes; `W` resolved to `R/.git` | yes with the plugin inside the repository (git resolved from its folder); with an installed plugin git found no repository from the server's folder, and only the probe's fixed path could be written | yes; `W` resolved to `R/.git` |
| `REPO_TASKS_DIR` and `REPO_TASKS_LIST` reach the server | yes, both | yes, both | yes, both | yes, both, with the server's own `environment` empty |
| They reach a session-start hook | yes, both | yes, both, in the hook process | yes, both | yes, both, in the plugin's hook process |
| Session-start event and the workspace path | `SessionStart` with `source` `startup`, `resume`, `compact`; payload keys `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `source`; `cwd` is the starting directory | `SessionStart` with `source` `startup` fired in all three places before any model call; payload keys `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `source`, `timestamp`, `transcript_path`; `cwd` is the starting directory | `SessionStart` once per run and `PreInvocation`; payload carries `cwd` and `workspacePaths`, both the starting directory; the hook's own working directory is the plugin folder | the plugin's `context` hook, on every model call, with `sessionID`; `ctx.location.directory` is the starting directory |
| A compaction is signalled | yes: `SessionStart` with `source` `compact`, after a three-turn session was compacted with `claude -p "/compact" --resume <id>` | not measured: a compaction needs a model call and no Qwen credential was used; read in the source at `v0.28.0` (`packages/core/src/hooks/types.ts:49-52`, `:1085-1091`; `chatCompressionService.ts:309-327`, `:1624`): `PreCompact` and `PostCompact` events and a `SessionStart` source `compact` exist | not measured: `hooks.md` names no compaction event and `agy --help` lists no compaction command | not measured: `opencode run "/compact"` was sent as ordinary text and no `compaction` hook line appeared; the only `compact` command in the source is the ACP catalog (`packages/cli/src/acp/catalog.ts:10`) |

Qwen Code 0.28.0 was installed for this (`npm install --prefix`, no scripts, no dependencies, into the
spike's own folder; not global), run with `QWEN_HOME` and the probe in its `settings.json`, and
stopped at "No auth type is selected": the server start, the hook and the MCP `roots` reply all
happen before that, so every cell above except compaction is measured, and no model credential
was used. Its source at tag `v0.28.0` (commit `895dfca8`) was read for the compaction events. Its
MCP client also declares `roots` and answered `roots/list` with the starting directory
(`file:///tmp/repo-tasks-spike/repo`, `.../repo/sub/deep`, `.../wt`).

Also not measured: OpenCode through its background service, because one was already running
(`opencode serve --service`) and is not the spike's to stop; a run against it would not load the
spike's configuration directory. A bare `claude -p "/compact"` on a fresh session answered "Not
enough messages to compact"; the compaction above needed a session of three turns.

Sections this contradicts or confirms:

- 7.1, Claude Code and OpenCode: the process working directory is the starting directory; the rule
  stands. **Antigravity contradicts it**: the server starts in the plugin folder, so "the process
  working directory" is the plugin's, not the workspace's, and the server cannot find the
  repository from it unless `REPO_TASKS_DIR` is set. A second test installed
  the probe as a plugin (`agy plugin install`, removed afterwards; the home configuration files
  were byte-identical before and after) and ran it from `R/sub/deep` three ways: the server's
  working directory was `~/.gemini/config/plugins/probe`, where git found no repository;
  `PWD` was the launching shell's value when kept, `/home/user` when the launcher set it so, and
  absent when unset, so it is inherited and says nothing about the workspace; no variable the
  server received named the workspace (only `PLUGIN_ROOT` and `PLUGIN_DATA` among the host's own);
  the hook payload still carried the workspace in `cwd` and `workspacePaths` in all three. So on
  Antigravity an installed server cannot find the repository from its process, and a rule that
  falls back to `PWD` would work only by accident. What remains is `REPO_TASKS_DIR` set in the
  host's environment (which is one value for all workspaces), something the server is told
  through the protocol, or leaving Antigravity's tools without a list. The protocol route was
  then measured (the roots test below): for the owner at step 2 (16). 8.5's start-up archiving resolves the list the same way and is affected the same
  way; 15.6's earlier unknown about the server's directory is settled for three of the four hosts.
- **The roots test** (2026-11-18, the same probe extended; Antigravity again as an installed and
  then removed plugin, config files byte-identical after). In all four measured hosts the client
  declares the MCP `roots` capability in `initialize` (Antigravity: `roots` with
  `listChanged: true`, client `antigravity-client` v1.0.0, also `elicitation`), and answers a
  `roots/list` request sent by the server after `notifications/initialized` with exactly one
  root, the starting directory as a `file://` URI: Claude Code `file:///tmp/repo-tasks-spike/repo/sub/deep`,
  OpenCode the same, Antigravity the same from `R/sub/deep` and `file:///tmp/repo-tasks-spike/wt`
  from the linked worktree; Qwen Code the same (`.../repo/sub/deep` and the worktree). So on Antigravity the server can learn the workspace without
  `REPO_TASKS_DIR`, from a request 7.1 does not yet mention. Not measured: a host with
  several roots; a root changing during a session (`listChanged` was declared by Antigravity,
  and no change was provoked); a client that declares no `roots`. The owner
  ruled on 2026-11-18 that 7.1 ranks the first root after `REPO_TASKS_DIR` and before the process
  working directory, and 8.5's start-up archiving waits for the reply to `roots/list`; both are
  now written there.
- 7.3 and 7.6: confirmed for the four measured hosts: each server could create and remove a
  folder in the git common directory, from `W` as well.
- 7.1 and 10.2, environment: confirmed for the server and the hook on the four measured hosts.
- 10.2, the Claude Code compaction source named `compact` [CLAIM, 2026-11-18]: confirmed.
- 10.2, Antigravity: confirmed; `workspacePaths` and `cwd` are both present, and a hook's working
  directory is the plugin folder.
- 10.2, OpenCode: confirmed; the event carries `sessionID` and `ctx.location.directory` is the
  starting directory.

## 16. Order of work

The plan written when this document is approved covers step 1 and nothing else.

1. **The spike** [approved, point 12]. It is throwaway and merges no product code. In each of the
   four agents, on this Linux machine, it records: the working directory of a started MCP server;
   whether that process can create a folder in the git common directory; whether
   `REPO_TASKS_DIR` and `REPO_TASKS_LIST`, set in the host's environment, reach the server and
   reach a session-start hook; which session-start event exists, and whether its payload carries
   the workspace path; whether a compaction is signalled. Its output is a table added to section
   15. Every cell holds what was observed, with the command or log line that showed it, or the
   words `not measured` and the reason. The implementation plan is not written while a cell for
   this machine says `not measured`, unless the owner has accepted that cell at step 2 (for
   example one that cannot be triggered from a script, such as a compaction).
2. **Revision.** Any section a measurement contradicts is revised, and the revision goes to the
   owner, before step 3. If a host cannot write under `.git`, sections 7.3 and 7.6 are the ones
   to revisit, and that is the owner's decision. If a host starts its server somewhere other
   than the workspace and its environment does not reach the server either, 7.1 has no way to
   find the repository on that host; 7.1 and the start-up archiving in 8.5, which resolves the
   list the same way before any tool call, are then the ones to revisit. (Outcome of step 2,
   2026-11-18: the spike found exactly that on Antigravity; the owner ruled that 7.1 and 8.5 take
   the client's first root, and accepted the cells 15.7 leaves `not measured`.)
3. **The implementation plan** is written only after steps 1 and 2, against code that exists then.

## 17. Left open

### 17.1 To the builder

- The MCP server library (as `shared/opencode-ctl` uses) and the crates for git discovery and
  hashing (`sha2` is already in `Cargo.lock`); whether the JSON is written compact or indented;
  error wording beyond what sections 7 to 10 fix.
- How OpenCode's `plugin.ts` learns the binary's absolute path. The precedents here are a
  configured value with an environment override (`opencode/plugins/rtk-mcp-opencode/plugin.ts:14`,
  `:86-87`) and a fixed install location (`opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:91`).
- The command file for each host that has commands (section 11).
- Test design. The rules in sections 7 to 10 are the oracle; 7.2 names its own.
- Each wrapper's README says where lists are kept (7.3, 7.5) and how to remove them. Uninstalling
  a plugin leaves them in place.
- The repository wiring a new plugin needs. The pull request that added `opencode-ctl` (merge
  `9f315e5`) touched, outside the plugin folders: `agents.json`; the four marketplace files
  (`.claude-plugin/`, `.qwen-plugin/`, `.agy-plugin/`, `.opencode-plugin/marketplace.json`);
  `release-please-config.json` and `.release-please-manifest.json`; `Cargo.toml` and
  `Cargo.lock`; `Justfile`; `.github/dependabot.yml`; `.github/workflows/ci.yml`,
  `footprint-freshness.yml` and `release-refresh.yml`; `docs/footprints/`; `README.md`; `QWEN.md`.

### 17.2 To the owner

1. **The name.** `repo-tasks` is the author's choice and every name in section 14 follows from it.
2. **Windows and macOS.** The engine's own rules are tested on all three systems by CI (15.2).
   How the four hosts start a server and a hook on Windows and macOS is not measured, and this
   machine is Linux. Is that a blocker for the first release? The author's recommendation: no;
   ship with those two systems marked unmeasured in each README and measure when a machine is
   available.
3. **A list that grows.** `task_list` prints every task and is never shortened (9.5), and one
   stale pending task stops automatic archiving. The author's recommendation: accept this in the
   first release and decide on a cap only if real lists get long.
4. **`todov2-opencode`.** The ROADMAP entry (section 12) is a decision for later, not part of
   this work.

### 17.3 The author's calls, in one place

Each is marked [driver's call] where it appears. None has been ruled on by the owner. One ruling
came after the spike and is not an author's call: the client's first root in 7.1 and 8.5, with
the cells 15.7 leaves `not measured` accepted (2026-11-18). Its details (one root read, a 2-second
wait, `file://` only) are the author's.

Four narrow or extend a point of the approved text, and need the owner's eye first:

1. Deletion applies only to an open task, and the deleted record is kept in the archive (9.4;
   point 7).
2. Start-up archiving waits until no task has changed for 12 hours (8.5; point 7).
3. `REPO_TASKS_DIR` wins over the event's path for the reminder too (7.1; point 9).
4. Line 1 of a list reply names the git common directory and the starting directory (9.5;
   point 4).

The rest fill gaps the approved text left:

5. The name `repo-tasks` and every name derived from it, binaries included (section 14).
6. The anti-goals (section 3).
7. The engine finds the git directory itself and never runs `git`; it ignores `GIT_DIR`,
   `GIT_COMMON_DIR` and `GIT_CEILING_DIRECTORIES`; a bare repository counts as not a repository;
   a `.git` entry that cannot be followed is an error (7.2).
8. Folder and file modes on Unix (7.3).
9. An invalid `REPO_TASKS_LIST` fails every call and is never altered to fit (7.4).
10. The per-user data directory on each system and the key derivation (7.5).
11. No control characters in a subject or an owner (8.1).
12. A malformed list file is never repaired or overwritten (8.3).
13. The whole version-skew policy (8.4).
14. `blocked_by_earlier` as the way a create call names its own earlier entries (9.2).
15. `task_get` answers from the archive for a task that has left the list (9.3).
16. `blocked_by` is information and never stops a status change (9.4).
17. One binary per wrapper with the hook as a subcommand; Antigravity's reminder never falls back
    to the process working directory; OpenCode's `plugin.ts` runs the binary, reminds once per
    session id, and never parses the list (10.2).
18. The command a person runs is named `repo-tasks` (section 11).
19. The reminder prints a notice, with no task text, when the list file cannot be used (10.1).
    Point 9 says only that it prints nothing when no task is open.
20. `reminder` takes `--workspace` and no `--dir` (7.1, section 11).
21. The list wins over the archive: an archive line for an id still in the list is ignored (8.5).
22. The forms of the `task_get` reply, the satisfied-blocker line and `list --archived` (9.2, 9.3,
    section 11).
23. A refused rename and a refused read are retried on Windows (8.2, 8.3).
24. A call that changes nothing succeeds and leaves `updated_at` alone; `remove_blocked_by`
    validation (9.4).
25. An archive append starts with a line feed after a torn line; archive readers skip any
    damaged line (8.5).
26. The reminder's notice also covers an unreadable list file and an unfollowable `.git`
    entry (10.1).

## 18. What done looks like

The owner opens Claude Code in a repository and asks for a five-step change. The model calls
`task_create` once with five tasks, the later ones naming the earlier ones in
`blocked_by_earlier` (9.2); the engine finds the git common directory (7.2), takes the lock,
writes `repo-tasks/default.json` and replies with five ids (8.2). The model marks task 1 and
task 2 completed and task 3 in_progress with `task_update` (9.4). The session is killed.

The owner opens OpenCode in a second worktree of the same repository. On the session's first
round the wrapper's `plugin.ts` runs `reminder` with the workspace directory (10.2); both
worktrees resolve to one git common directory (7.2), so the line says three tasks are open, one
in progress (10.1), and names no task. The model calls `task_list` and reads, under line 1, five
task lines: two
completed, task 3 in_progress and marked as last changed by `claude-code` with its time (9.5),
tasks 4 and 5 blocked. It takes task 3 over with an ordinary update (9.4), and deletes task 5,
which is no longer wanted; task 5 moves to the archive (8.5).

When the other four are completed they stay in the list, and an agent opened an hour later still
sees them. The next day, more than 12 hours after the last change, an agent's MCP server starts,
finds every task completed, and moves them to `default.archive.jsonl` (8.5). The owner can print
all five with `list --archived` (section 11).

## Appendix A. Review ledger

### Owner rulings (verbatim, 2026-11-18)

| Ruling | What it settled |
|---|---|
| "Build a TodoWrite replacement like Tasks" | the request |
| "yes" | "Tasks" means Claude Code's task tools |
| "I'm going to ship it as well" | it is a published plugin |
| "Claude Code for me, ship for all four" | the hosts |
| "same list for any agent, and yes run the checks" | the list belongs to the repository; three read-only checks allowed |
| "Negotiate with agy and present me a common agreed recommendation" | one jointly signed text instead of a choice between options |
| "approved, write the design doc" | the fourteen points of section 6 |

### Consult round 1 (the shape and the store location)

| Finding | Disposition |
|---|---|
| A working-tree folder is deleted by `git clean -fdx`, split across worktrees, and can arrive in a clone | FOLDED; each measured (15.1) |
| A working-tree folder dirties `git status` | REJECTED: `git status --short` prints nothing (15.1) |
| One file per list under an OS lock, temp file and rename | FOLDED; pattern confirmed at `pairing.rs:282`, `:329`, `:427` |
| Option E, an OpenCode-native build | REJECTED: it cited lines 48-107 of `opencode/plugins/todov2-opencode/plugin.ts`, a 46-line file, and an API name that occurs nowhere in it |
| Hosts keep their own list tools beside the tracker | FOLDED as point 10 |
| A task left in_progress by a dead session needs a lease or process id | Not adopted; point 8 records who changed a task and when, and the peer agreed to it |

### Consult round 2 (three store locations)

| Finding | Disposition |
|---|---|
| The git common directory ranks first, the per-user folder second, the working tree last | FOLDED as point 2 |
| Fall back to the per-user folder when `.git` is not writable | REJECTED by the author (5.3); the peer agreed in the agreement round |
| Antigravity states no restriction on where a started process writes | FOLDED as a statement about its documentation only (15.5) |
| `git gc`, `git prune` and worktree removal leave an unknown folder alone | Was a hypothesis; measured afterwards (15.1) |

### Agreement round (AGREE 11, AMEND 2, DISSENT 0)

| Finding | Disposition |
|---|---|
| Point 9: Qwen and OpenCode already have session-start patterns in this repository; both reviewers had missed them | FOLDED; the files confirmed (10.2) |
| Point 7: deleting a task leaves ghost blockers when only "blocked by" is stored | FOLDED |
| Point 7: create should accept blockers, including earlier entries of the same call | FOLDED |
| Point 7: a flag on create that clears completed tasks | REJECTED by the author with a traced case (5.4); the peer agreed |
| OpenCode may start a server in the plugin folder | REJECTED for 2.3.0 on a reading of its source (15.4); still in the spike |

### Sign-off rounds (AGREE 3, then AGREE 1)

| Finding | Disposition |
|---|---|
| Archive-on-next-create collides with rejecting a missing blocker | FOLDED: archive only at MCP server start; a blocker that left the list counts as satisfied |
| One stale pending task stops automatic archiving | Accepted as a known limit by both reviewers; now an owner question (17.2, item 3) |
| A second agent starting while every task is completed archives them out of a running session's view | Was below the floor; now FOLDED by the 12-hour condition (8.5) |

### Builder check (2026-11-18, a fresh reader with none of the conversation, read-only)

| Finding | Disposition |
|---|---|
| `delete: true` lets any agent erase any task, against the anti-goal and against "two ways only" in point 7 | FOLDED: deletion only on an open task, record kept in the archive (9.4, 8.5); the anti-goal reworded |
| The reminder and the server resolve D in different orders, so they can count different lists | FOLDED: `REPO_TASKS_DIR` first everywhere (7.1); 10.2 now defers to 7.1 |
| A task archived under a running session just errors; any server start archives; "the next day" was not a rule | FOLDED: 12-hour condition (8.5); `task_get` answers from the archive (9.3); errors say a task has left the list (9.1) |
| `task_list` grows without bound | Put to the owner (17.2, item 3); the reply states it is never shortened (9.5) |
| Is Windows and macOS correctness a release gate | Put to the owner (17.2, item 2); CI coverage recorded (15.2) |
| What the first plan covers | FOLDED: section 16 opens with it |
| The cited lock call blocks with no deadline | FOLDED: 8.2 step 1 now asks for non-blocking attempts until a deadline |
| Line 1 of a list reply shows the git common directory, not the path the call started from | FOLDED: line 1 shows both (9.5) |

### Panel round 1, solo (2026-11-18)

Persona: relentless adversarial auditor. Seated: Axiom Breaker, Cascade Analyst, State Corruptor,
Protocol Pedant, Literal Implementer, Boundary Smuggler. Not yet seated, kept for later rounds:
Blindspot Auditor, Activation Auditor, Dependency Cynic, Resource Vampire, Mechanism Gamer.

| Seat | Finding | Disposition |
|---|---|---|
| Axiom Breaker | Reminder and server orders differ (also found by the builder check) | FOLDED (7.1) |
| Axiom Breaker | OpenCode's one-shot value is per plugin load, not per session | FOLDED: once per session id (10.2) |
| Cascade Analyst | A crash leaves a temp file behind for ever | FOLDED: fixed temp name, removed under the lock (8.2) |
| Cascade Analyst | A failed rename, and a power cut after a rename, were not covered | FOLDED: failure rule and folder flush (8.2) |
| Cascade Analyst | The start-up archive could hold up a server waiting for the lock | FOLDED: one non-blocking attempt (8.5) |
| Cascade Analyst | A `.git` entry that leads nowhere fell through to the non-git case | FOLDED: it is an error (7.2) |
| State Corruptor | An update naming an id that has left the list had no rule | FOLDED (9.1, 9.4) |
| Protocol Pedant | Whether an id may be a string; what a reply is; what counts as a character; the values of `--event` | FOLDED (9.1, 8.1, section 11) |
| Literal Implementer | Binary names and the hook command were not stated | FOLDED (section 14, 10.2) |
| Boundary Smuggler | A crafted `.git` file could point the store at any directory | FOLDED: G must contain `HEAD` (7.2) |
| Boundary Smuggler | A subject could carry terminal escape sequences into a person's terminal | FOLDED: no control characters (8.1) |
| Boundary Smuggler | Text one agent writes is read by another through the tools | Stated as the trust model (section 12); no mechanism added |

### Panel, second solo pass (2026-11-18)

Seated, none of them used in the first pass: Blindspot Auditor, Activation Auditor, Dependency
Cynic, Resource Vampire, Mechanism Gamer; plus an audit of the first pass's own folds.

| Seat | Finding | Disposition |
|---|---|---|
| Fold audit | "No task changed within the last 12 hours" let a timestamp in the future pass the test | FOLDED: every `updated_at` must be at least 12 hours in the past (8.5) |
| Fold audit | The start-up archive took the lock first, creating a folder and a lock file in a repository with no list | FOLDED: read first, lock only when the condition holds (8.5) |
| Mechanism Gamer | "No tool call destroys a record" was false as written: update rewrites any field | FOLDED: the anti-goal now says no task disappears without trace (section 3) |
| Mechanism Gamer | The 3,690-byte stop line depended on someone remembering it | FOLDED: the plan carries a failing check (section 13) |
| Mechanism Gamer | The spike gate could be met by leaving a measurement out | FOLDED: every cell is filled or says `not measured`, and that blocks the plan (section 16) |
| Blindspot Auditor | No word on recovering a malformed list, moving a task back from the archive, or what an uninstall leaves behind | FOLDED as stated limits (8.3, section 11, 17.1) |
| Activation Auditor | A command named after tasks or todos can collide with a host's own | FOLDED: it is named `repo-tasks` (section 11) |
| Dependency Cynic | A `.git` file can end in a carriage return; `commondir` can be absolute; a Windows path shows its `\\?\` prefix | FOLDED (7.2) |
| Dependency Cynic | Advisory locks are not honoured on every filesystem | Accepted limit (Stand-downs) |
| Resource Vampire | The archive grows for ever | Stated: the engine never trims it (8.5) |
| Resource Vampire | The list reply grows for ever | Already put to the owner (17.2, item 3) |

### Panel round 1, the peer (2026-11-18, a new chat with none of the earlier context)

Seated: Fold Auditor, Axiom Breaker, Cascade Analyst, State Corruptor, Protocol Pedant, Literal
Implementer, Procedure Walker (new). The brief is `.clavity/seams/repo-tasks-design-panel-r1.md`;
the peer returned FINDINGS-14. Each was checked against the document or the cited file.

| Seat | Finding (the peer's severity) | Disposition |
|---|---|---|
| Fold Auditor | A deletion whose list rewrite fails leaves a `deleted_at` line for a task still in the list (MATERIAL) | FOLDED: the list wins; archive readers ignore a line whose id is in the list (8.5). The peer's fix, cutting the archive back, REJECTED: the archive is append-only |
| Fold Auditor | `reminder --dir` from OpenCode against "`REPO_TASKS_DIR` first" (MATERIAL) | FOLDED: `reminder` takes `--workspace`, ranked as the event's path, and has no `--dir` (7.1, 10.2, section 11) |
| Fold Auditor | 9.1 names two error cases, 9.3 a third (MINOR) | FOLDED (9.1) |
| Axiom Breaker | Start-up archiving assumes the server starts in the repository (BLOCKING) | REJECTED as a new defect: the working directory belongs to the process, so the start-up resolution is the one every tool call gets, and a wrong one is the unmeasured item already in 15.6. Its consequence FOLDED into section 16, step 2 |
| Axiom Breaker | A completed task can be reopened and then deleted (MATERIAL) | Stated as a known limit (9.4, Stand-downs); the record is kept either way. The peer's fixes not adopted: both forbid reopening finished work |
| Cascade Analyst | A malformed list makes the reminder silent while every tool fails (MATERIAL) | FOLDED: a text-free notice line (10.1, 8.3). No repair command is added; 8.3 says so |
| Cascade Analyst | A crash during archiving duplicates archive lines (MINOR) | REJECTED: 8.5 already stated it and the last-line rule that makes it harmless |
| State Corruptor | On Windows a reader holding the file open makes the writer's rename fail (BLOCKING) | FOLDED as an unmeasured claim: a bounded retry, how readers open the file, and a CI test on Windows as the measurement (8.2, 8.3, 15.6). The existing `pairing.rs:329` has no retry |
| State Corruptor | `task_get` can read a half-written archive line (MINOR) | REJECTED for the id asked about: the archive line is flushed before the list drops the task. The read order that makes this true is now stated (8.3) |
| Protocol Pedant | A bare string pushed onto OpenCode's `event.system` (MATERIAL) | FOLDED: the object form, confirmed at `re-ghidra-mcp-opencode/plugin.ts:204` and `todov2-opencode/plugin.ts:20` (10.2) |
| Protocol Pedant | `task_get`'s reply had no fixed order or form (MINOR) | FOLDED: a template (9.3) |
| Protocol Pedant | The "accepted as satisfied" line had no form (MINOR) | FOLDED (9.2, 9.4) |
| Literal Implementer | `list --archived` had no header, order or marks (MATERIAL) | FOLDED (section 11) |
| Literal Implementer | How OpenCode's wrapper finds the binary is left to the builder (MINOR) | Left to the builder as before; the two precedents are now named (17.1) |
| Procedure Walker | Section 18 says "five lines" where 9.5 gives six | FOLDED (section 18) |
| Question 2 | 9.1 (an error) and 9.6 (accepted) read as one rule stated twice | FOLDED: 9.1 now says it covers the id a call acts on, and 9.6 covers blockers |

The peer named as least examined: host behaviour on Windows and macOS, and the two outside
sources beyond the cited lines.

### Panel round 2, the peer (2026-11-18, same chat)

Seated: Fold Auditor, Axiom Breaker, Mechanism Gamer, Dependency Cynic, Blindspot Auditor (the
last three new), Procedure Walker. The brief is `.clavity/seams/repo-tasks-design-panel-r2.md`;
the peer returned FINDINGS-13. It also re-read the three round 1 findings I had rejected: all
three fell, on its reading of the lines. Each finding was checked against the document or file.

| Seat | Finding (the peer's severity) | Disposition |
|---|---|---|
| Fold Auditor | A torn archive line fuses with the next append and is then not a final line (MATERIAL) | FOLDED: an append starts with a line feed after a torn line, and any damaged line is skipped (8.5). Its claim that readers then fail was wrong: 8.3 governs the list only, now said |
| Fold Auditor | OpenCode's invocation omits `--event` (MINOR) | FOLDED (10.2) |
| Fold Auditor | `task_get` can never return the "left the list" case (MINOR) | FOLDED (9.1) |
| Fold Auditor | The rename retry deadline is what is left of the lock budget (MINOR) | FOLDED: its own 5 seconds from the first refusal (8.2) |
| Axiom Breaker | `remove_blocked_by` has no validation rule (MATERIAL) | FOLDED (9.4) |
| Axiom Breaker | A no-op update moves `updated_at` and holds off archiving (MATERIAL) | FOLDED: a call that changes nothing writes nothing (9.4) |
| Mechanism Gamer | A Linux-only spike gate passes with Windows and macOS unmeasured (MATERIAL) | REJECTED as already put to the owner (17.2, item 2). A cell that cannot be measured from a script may now be accepted by the owner (16) |
| Mechanism Gamer | One non-blocking attempt at start-up skips archiving for a long session (MINOR) | REJECTED below the floor: only housekeeping is delayed; the next start and the `archive` subcommand retry |
| Dependency Cynic | A Windows rename over an open file may fail as `ERROR_ACCESS_DENIED`, not a sharing violation (BLOCKING) | FOLDED as MATERIAL: still an unmeasured claim; the retried error kinds are now named and the test records which appears (8.2, 8.3). Severity not accepted: nothing is built on it before the CI measurement |
| Dependency Cynic | Differently cased Windows paths give two lists (MATERIAL) | FOLDED as an unknown: canonicalization may already return one spelling; a Windows CI test decides (7.5, 15.6). Lower-casing not adopted |
| Dependency Cynic | No locking crate is named (MINOR) | REJECTED: `pairing.rs:427` calls `lock.lock()` on a `std::fs::File` and the file imports no locking crate |
| Blindspot Auditor | The reminder prints nothing on a permission or `.git` failure (MATERIAL) | FOLDED: the notice of 10.1 now covers both |
| Blindspot Auditor | A start-up archiving failure goes to standard error that nobody sees (MINOR) | REJECTED below the floor: nothing is lost, the next start retries |
| Procedure Walker | Section 18 reached sentence 11 | no finding |
| Question 2 | The spike gives no way to trigger a compaction (peer) | FOLDED in part: a cell the owner accepts as unmeasurable no longer blocks the plan (16); the method belongs to the spike's own plan |

### Panel round 3, the peer (same chat)

Seated: Fold Auditor, State Corruptor, Protocol Pedant, Literal Implementer (the last three new),
Axiom Breaker, Procedure Walker. The brief is `.clavity/seams/repo-tasks-design-panel-r3.md`; the
peer returned FINDINGS-3, all MINOR. Fold Auditor, State Corruptor, Axiom Breaker and Procedure
Walker found nothing.

| Seat | Finding (the peer's severity) | Disposition |
|---|---|---|
| Protocol Pedant | The order of satisfied-blocker lines in a create reply (MINOR) | FOLDED (9.2) |
| Protocol Pedant | `archive` output had no form (MINOR) | FOLDED (section 11) |
| Literal Implementer | Antigravity's existing hook reads `workspacePaths`, the document says `cwd` | FOLDED, checked in `hook.rs:89-96` and `hooks.md:159`: both are in the payload; `workspacePaths` first, else `cwd` (10.2, 15.5) |
| Question 2 | 10.1 says "when anything fails" prints nothing, then adds a notice | FOLDED: "anything else fails" |
| Question 1 | A writer retrying `PermissionDenied` waits 5 seconds on a real read-only file | REJECTED: bounded and stated in 8.2; the cost of a real permission fault is 5 seconds, once |

PANEL VERDICT: round 2 complete (the peer returned FINDINGS-13, all disposed above); round 3 returned three MINOR findings, folded below it; a fourth round over those folds is owed.

### Panel round on the roots revision (2026-11-18, the peer, same chat)

The brief is `.clavity/seams/repo-tasks-roots-panel-r1.md`; the peer returned FINDINGS-7, each
checked against the text.

| Seat | Finding (the peer's severity) | Disposition |
|---|---|---|
| Fold Auditor | The working-directory fallback on Antigravity opens a list for the plugin's folder (MATERIAL) | FOLDED: no fallback on Antigravity; the call fails and names `REPO_TASKS_DIR` (7.1), as 10.2 already says for the reminder |
| Fold Auditor | 8.5 used a 2-second settlement that 7.1 defined only for a waiting tool call (BLOCKING) | FOLDED: a request is settled by its reply or 2 seconds after it was sent; 8.5 acts only on an answered request (7.1, 8.5) |
| Fold Auditor | Windows `file://` forms beyond `C:` (MATERIAL) | FOLDED as RFC 8089 reading, marked [CLAIM] (7.1) |
| State Corruptor | A call before `initialized` skips the wait (MATERIAL) | FOLDED: refused with an error, no list touched (7.1) |
| State Corruptor | Every call waits again after a timeout (MATERIAL) | FOLDED: a settled request is not waited for again (7.1) |
| Literal Implementer | A refresh that times out: keep or clear the root (MINOR) | FOLDED: the root held is kept (7.1) |
| Blindspot Auditor | Start-up archiving after a timeout acts on the working directory's list (MATERIAL) | FOLDED: a timeout-only settlement skips start-up archiving (8.5) |

Round 2 on that revision (the peer returned FINDINGS-7, each checked against the text), all
FOLDED in 7.1 and 8.5: a timed-out first request counts as "no root"; the URI is converted before
the directory test; any Windows drive letter; the error for a call before `initialized` is a
JSON-RPC error, code -32600 [driver's call]; a call resolves D once and keeps it; the 2 seconds
can be shortened by tests only; start-up archiving is decided once and a late reply does not
re-arm it. None rejected.

Round 3 on that revision (the peer returned FINDINGS-3, each checked): FOLDED in 7.1 a late reply to
the first request is used only before any call has resolved D, and one request is in flight at a
time with a notification during it remembered; REJECTED below the floor: a timeout-only
settlement skips start-up archiving for the whole session (8.5) — only housekeeping is delayed,
the next start tries again, and the `archive` subcommand exists.

Round 4 on that revision (the peer returned FINDINGS-3, each checked): FOLDED in 7.1 without a
fifth round, as they were one missing clause and the tests it needs: a remembered notification is
served after a timeout as after a reply, and the tests for these rules are listed in both
directions. No round on this revision was clean; the owner may ask for one more.

## Stand-downs

- Accepted limit: one stale pending task stops automatic archiving (17.2, item 3).
- Accepted limit: outside a git repository, a subdirectory opens a different list (7.5). This one
  was never put to the peer.
- Accepted, no mechanism: text written by one agent is read by another through the tools
  (section 12).
- Accepted limit: the two-writer guarantee rests on OS advisory locks. A repository on a
  filesystem that does not honour them (some network mounts) loses it.
- Accepted limit: a completed task can be reopened and then deleted in two calls (9.4). Its
  record stays in the archive.
- Unverified and carried into the spike: everything in 15.6.
- One clause of point 7 was written after the peer's last sign-off (end of section 6), and the
  four narrowings in 17.3 were written after the owner approved the text.
