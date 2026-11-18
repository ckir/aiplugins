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
- A model can remove another session's history with one argument.
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

## 7. Finding the list

### 7.1 The starting directory

Every entry point resolves a starting directory D, then a list from D (7.2 to 7.5).

| Entry point | D is the first of these that is present |
|---|---|
| MCP server, on each tool call | `REPO_TASKS_DIR`; the process working directory |
| Reminder (section 10) | the workspace path in the host's event; `REPO_TASKS_DIR`; the process working directory, except on Antigravity (10.2) |
| A subcommand run by a person (section 11) | the `--dir` argument; `REPO_TASKS_DIR`; the process working directory |

`REPO_TASKS_DIR` counts as present when it is set and not blank. It must be an absolute path to an
existing directory; otherwise every call fails with an error naming the variable and its value.
The server reads the working directory on each call, not once at start [approved, point 4].

### 7.2 The git common directory

Walk up from D to the filesystem root. At the first directory that has an entry named `.git`:

- if `.git` is a directory, that directory is the git directory G;
- if `.git` is a file whose content starts with `gitdir: `, G is the path after that prefix,
  resolved against the directory holding the file.

The git common directory C is G joined with the content of the file `G/commondir` when that file
exists (a path relative to G), and G itself otherwise. C is then made canonical and absolute.

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
  folding is applied.

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
- `subject` is required, 1 to 200 characters after trimming, with no line break. `description`
  and `owner` are optional and are omitted, never null, when absent. `description` is at most
  8,000 characters; `owner` is at most 64 characters with no line break. The three limits are
  [PROPOSED, UNMEASURED].
- `status` is one of `pending`, `in_progress`, `completed`.
- `blocked_by` is an array of ids, ascending, without duplicates, never containing the task's own
  id, and containing only ids of tasks present in the list. It may name completed tasks.
- `created_at` and `updated_at` are strings in RFC 3339 form, UTC, whole seconds, ending in `Z`.
  They are strings, never numbers.
- `updated_by` names who made the last change: `claude-code`, `qwen`, `antigravity`, `opencode`
  (the wrapper's compiled-in host, narrowed the way `shared/opencode-ctl/src/host.rs` narrows it
  for OpenCode), or `cli` for a subcommand run by a person.

### 8.2 Writing [approved, point 6]

Every change follows the same five steps:

1. Open `N.lock`, creating it if needed, and take an exclusive OS advisory lock on it (the call at
   `shared/opencode-ctl/src/pairing.rs:427`). The operating system releases the lock when the
   process dies. A writer that does not get the lock within 5 seconds [PROPOSED, UNMEASURED]
   fails the call; it never proceeds without the lock.
2. Read `N.json`. A missing file is the empty list: format 1, next id 1, no tasks.
3. Check and apply the whole change in memory. If any rule in sections 8 or 9 is broken, fail the
   call and leave the file untouched. A call changes everything it asked for or nothing.
4. Write the new content to a temp file in the same folder, flush it to disk, and rename it over
   `N.json` (`pairing.rs:282` and `:329` show the pattern).
5. Release the lock.

### 8.3 Reading

`task_list`, `task_get`, the reminder and the checklist subcommand take no lock. The rename in
step 4 means a reader sees the whole old file or the whole new file.

A file that is not valid JSON, or breaks a rule in 8.1, is never repaired or overwritten
[driver's call]. Every call fails with an error naming the file, except the reminder, which
prints nothing.

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
`"archived_at"` in the form of 8.1. It is only ever appended to.

Archiving a set of completed tasks, under the lock of 8.2:

1. Append their lines to the archive and flush it to disk.
2. Rewrite the list without them, removing their ids from every remaining task's `blocked_by`.

A crash between the two steps leaves a task in both files; the next archiving appends it again.
A reader of the archive uses the last line for an id and ignores a final line that is not
complete JSON.

There are exactly two triggers:

- **MCP server start.** When `serve` starts, it resolves the list as a tool call would. If the
  list holds at least one task and every task is completed, it archives them all. Any failure
  leaves the list as it is, writes one line to standard error, and the server starts normally.
- **A person.** The `archive` subcommand (section 11) archives every completed task.

No tool call, no reminder and no other subcommand archives anything. `next_id` is unchanged by
archiving.

## 9. The tools

The server exposes four tools. Names and parameters are a contract.

### 9.1 `task_create`

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
- `blocked_by` names ids already issued (9.5). `blocked_by_earlier` names earlier entries of this
  same call by 1-based position, and each value must be lower than the entry's own position
  [driver's call on the encoding].
- New tasks start as `pending`.
- Reply: one line per task, `created #<id> <subject>`, followed by one line for each blocker that
  was accepted as already satisfied.

### 9.2 `task_get`

`{ "id": 3 }`. Reply: the members of the task one per line as `name: value`, with `blocked_by` in
full, a derived line `blocks:` listing the ids of tasks whose `blocked_by` contains this id, and
the description last after a blank line. An id not in the list is an error.

### 9.3 `task_update`

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

- Only `id` is required; at least one other member must be present.
- An empty `owner` or `description` removes that member.
- `delete: true` removes the task and removes its id from every other task's `blocked_by`. It
  cannot be combined with any other change.
- Any status may follow any status. The engine does not stop a task starting or completing while
  a blocker is open: `blocked_by` is information for the reader, not a gate [driver's call].
- Any agent may update any task [approved, point 8].
- Reply: `updated #<id>` or `deleted #<id>`, plus a line per blocker accepted as satisfied.

### 9.4 `task_list`

No parameters. Reply, as plain text:

```text
repo-tasks: /home/ck/src/widget/.git (list default)
#1 [x] Reproduce the failing upload [owner ck]
#2 [>] Fix the chunk size [claude-code 2026-11-18T10:41:07Z]
#3 [ ] Add a regression test [blocked by #2]
```

- Line 1 is `repo-tasks: <C> (list <N>)`. Outside a git repository `<C>` is canonical D followed
  by ` (not a git repository)`.
- Then one line per task in id order: `#<id>`, a mark (`[ ]` pending, `[>]` in_progress, `[x]`
  completed), the subject, then in this order when they apply: `[owner <owner>]`;
  `[blocked by #a, #b]` listing only blockers that are not completed; and for an in_progress task
  `[<updated_by> <updated_at>]`, which is how a task left by a dead session shows.
- An empty list prints line 1 and then `no tasks`.

### 9.5 Blockers and cycles [approved, point 7]

For each id named in `blocked_by` or `add_blocked_by`:

- the id is in the list: it is stored;
- the id is at least 1, below `next_id`, and not in the list: it is accepted as already
  satisfied, not stored, and the reply says so;
- the id is below 1, or at or above `next_id`: the call fails.

After a change is applied in memory, the `blocked_by` relation over the tasks in the list must
contain no cycle, and no task may block itself; otherwise the call fails.

### 9.6 Errors

A failed call returns an MCP tool error whose text is one line saying what was refused and why. A
failed call changes nothing (8.2, step 3).

## 10. The reminder [approved, point 9]

### 10.1 What it says

Exactly one line, with n the number of tasks that are pending or in_progress and m the number
that are in_progress:

```text
repo-tasks: <n> open in this repository's task list, <m> in progress. Call the task_list tool to see them.
```

It contains no subject, description or owner. When n is 0, when there is no list, or when
anything fails, it prints nothing and still exits successfully: a reminder never blocks or fails
a session.

The line is produced by the engine binary's `reminder` subcommand and by nothing else. No wrapper
reads the list file itself.

### 10.2 Per host

| Host | When it runs | How the line reaches the model | Where D comes from |
|---|---|---|---|
| Claude Code | `SessionStart` for the sources `startup`, `resume`, `clear`, and the compaction source | `hookSpecificOutput.additionalContext`, as `claude-code/plugins/re-ghidra-mcp-cc/src/bin/hook.rs:78-80` | event `cwd`, then `CLAUDE_PROJECT_DIR`, then the working directory, as that file's lines 41-43 |
| Qwen | `SessionStart` for `startup`, `resume`, `clear` (`qwen/plugins/re-ghidra-mcp-qwen/qwen-extension.json:17-43`) | `systemMessage` and `hookSpecificOutput.additionalContext`, as `qwen/plugins/re-ghidra-mcp-qwen/src/bin/hook.rs:77-80` | the event's workspace path, as that hook reads it |
| Antigravity | `PreInvocation`, acting only when `invocationNum` is at most 1 (the check in `antigravity/plugins/re-ghidra-mcp-agy/src/bin/hook.rs`) | `injectSteps`, as that file's line 78 | `cwd` on standard input only |
| OpenCode | once per session through a one-shot value in the `context` hook, and again from the `compaction` hook (the `Plugin.define` block of `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts`) | pushed onto `event.system` | `ctx.location.directory`, passed to the binary as `--dir` |

Two host rules narrow point 9 [driver's call]:

- **Antigravity never falls back to the process working directory.** Its documentation says a
  hook's working directory is the folder holding `hooks.json`, which is the plugin, not the
  workspace. With no `cwd` in the event the reminder prints nothing.
- **OpenCode's `plugin.ts` runs the binary** (as `opencode/plugins/rtk-mcp-opencode/plugin.ts`
  runs its own with `spawnSync`) and pushes whatever line comes back. It contains no code that
  parses the list.

The name of Claude Code's compaction source (`compact` [CLAIM, 2026-11-18, from the author's
memory of the documentation]) and whether Qwen and Antigravity signal a compaction at all are for
the spike (section 16). Where a host signals none, the reminder runs at session start only.

## 11. The command line [approved, points 7 and 11]

Each wrapper ships one binary that contains the engine. Its subcommands:

| Subcommand | Does |
|---|---|
| `serve` | runs the MCP server on standard input and output (OpenCode adds `--host opencode`, as `opencode-ctl` does) |
| `reminder --event <host>` | prints the line of 10.1, wrapped the way that host expects, or nothing |
| `list` | prints exactly what `task_list` replies; `list --archived` prints the archive in the same line form |
| `archive` | archives every completed task (8.5) and says how many |
| `--version` | prints the version |

`reminder`, `list` and `archive` accept `--dir <path>` (7.1). A change made by `archive` records
`cli` as `updated_by`.

Each wrapper also ships a command that runs `list`, on every host that has commands. This
repository has command folders for OpenCode and Qwen today
(`opencode/plugins/aiplugins-devtools-opencode/commands`, `qwen/plugins/example/commands`); the
form for Claude Code and whether Antigravity has one are the builder's to settle.

## 12. Hosts' own lists [approved, point 10]

The tracker adds four tools and removes or changes nothing a host provides. Each tool description
carries, once, the sentence that tells a model when to choose it: use these tools for work that
must outlive the session or be seen by another agent.

`opencode/plugins/todov2-opencode` is not touched. OpenCode 2.3.0 ships a built-in `todowrite`
again as an opt-in (section 15.4), under the same tool name the plugin registers. The first pull
request of the implementation adds a ROADMAP entry recording that overlap for the owner to rule
on.

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

## 14. Interfaces

Everything in this table is a contract: other processes, other engine versions or people depend
on it.

| Thing | Name |
|---|---|
| Engine crate | `shared/repo-tasks` |
| Wrapper plugins | `claude-code/plugins/repo-tasks-mcp-cc`, `qwen/plugins/repo-tasks-mcp-qwen`, `antigravity/plugins/repo-tasks-mcp-agy`, `opencode/plugins/repo-tasks-mcp-opencode` |
| MCP tools | `task_create`, `task_get`, `task_update`, `task_list` (section 9) |
| Environment variables | `REPO_TASKS_DIR` (7.1), `REPO_TASKS_LIST` (7.4) |
| Store folder | `<git common directory>/repo-tasks/` (7.3); outside git `<per-user data directory>/repo-tasks/<key>/` (7.5) |
| Files | `<list>.json`, `<list>.lock`, `<list>.archive.jsonl`, and outside git `directory.txt` |
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
  workspace arrives as `"cwd"` on standard input (`hooks.md:165`).
- `SessionStart` and `PreInvocation` events exist. No restriction on where a started process may
  write is stated.

### 15.6 Not measured

- For Claude Code, Qwen and Antigravity at run time: the working directory given to an MCP
  server, whether it may write under `.git`, and whether the host environment reaches it. For
  OpenCode these rest on a reading of the source, not a run.
- Whether `REPO_TASKS_DIR` and `REPO_TASKS_LIST` reach a hook process as well as the server.
- Whether Claude Code signals a compaction to a `SessionStart` hook, and under what name.
- Any hook event of Qwen beyond those this repository already uses.
- Windows and macOS. Every measurement in 15.1 ran on one Linux machine.
- Whether `CLAUDE_CODE_ENABLE_TODO_TOOLS` switches Claude Code's own tools back on.
- Every figure marked [PROPOSED, UNMEASURED].

## 16. Order of work

1. **The spike** [approved, point 12]. The first plan covers only this. It is throwaway and
   merges no product code. In each of the four agents it records: the working directory of a
   started MCP server; whether that process can create a folder in the git common directory;
   whether `REPO_TASKS_DIR` and `REPO_TASKS_LIST`, set in the host's environment, reach the
   server and reach a session-start hook; which session-start event exists, and whether its
   payload carries the workspace path; whether a compaction is signalled. Its output is a table
   added to section 15.
2. **Revision.** Any section a measurement contradicts is revised, and the revision goes to the
   owner, before step 3. If a host cannot write under `.git`, sections 7.3 and 7.6 are the ones
   to revisit, and that is the owner's decision.
3. **The implementation plan** is written only after steps 1 and 2, against code that exists then.

## 17. Left open

### 17.1 To the builder

- The crates used for git discovery and hashing (`sha2` is already in `Cargo.lock`); whether the
  JSON is written compact or indented; error wording beyond what sections 7 to 10 fix.
- How OpenCode's `plugin.ts` learns the binary's absolute path.
- The command file for each host that has commands (section 11).
- Test design. The rules in sections 7 to 10 are the oracle; 7.2 names its own.
- The repository wiring a new plugin needs. The pull request that added `opencode-ctl` (merge
  `9f315e5`) touched, outside the plugin folders: `agents.json`; the four marketplace files
  (`.claude-plugin/`, `.qwen-plugin/`, `.agy-plugin/`, `.opencode-plugin/marketplace.json`);
  `release-please-config.json` and `.release-please-manifest.json`; `Cargo.toml` and
  `Cargo.lock`; `Justfile`; `.github/dependabot.yml`; `.github/workflows/ci.yml`,
  `footprint-freshness.yml` and `release-refresh.yml`; `docs/footprints/`; `README.md`; `QWEN.md`.

### 17.2 To the owner

- The name. `repo-tasks` is the author's choice and every name in section 14 follows from it.
- When and where Windows and macOS are measured; this machine is Linux.
- The ROADMAP entry on `todov2-opencode` (section 12) is a decision for later, not part of this
  work.

### 17.3 The author's calls, in one place

Each is marked [driver's call] where it appears. None has been ruled on by the owner.

1. The name `repo-tasks` and every name derived from it (section 14).
2. The anti-goals (section 3).
3. The engine finds the git directory itself and never runs `git`; it ignores `GIT_DIR`,
   `GIT_COMMON_DIR` and `GIT_CEILING_DIRECTORIES`; a bare repository counts as not a repository (7.2).
4. Folder and file modes on Unix (7.3).
5. An invalid `REPO_TASKS_LIST` fails every call and is never altered to fit (7.4).
6. The per-user data directory on each system and the key derivation (7.5).
7. A malformed list file is never repaired or overwritten (8.3).
8. The whole version-skew policy (8.4).
9. `blocked_by_earlier` as the way a create call names its own earlier entries (9.1).
10. `blocked_by` is information and never stops a status change (9.3).
11. Antigravity's reminder never falls back to the process working directory, and OpenCode's
    `plugin.ts` runs the binary and never parses the list (10.2).

## 18. What done looks like

The owner opens Claude Code in a repository and asks for a five-step change. The model calls
`task_create` once with five tasks, the later ones naming the earlier ones in
`blocked_by_earlier` (9.1); the engine finds the git common directory (7.2), takes the lock,
writes `repo-tasks/default.json` and replies with five ids (8.2). The model marks task 1 and
task 2 completed and task 3 in_progress with `task_update` (9.3). The session is killed.

The owner opens OpenCode in a second worktree of the same repository. The wrapper's `plugin.ts`
runs `reminder` with the workspace directory (10.2); both worktrees resolve to one git common
directory (7.2), so the line says three tasks are open, one in progress (10.1), and names no
task. The model calls `task_list` and reads five lines: two completed, task 3 in_progress and
marked as last changed by `claude-code` with its time (9.4), tasks 4 and 5 blocked. It takes
task 3 over with an ordinary update (9.3).

When all five are completed they stay in the list. The next day an agent's MCP server starts,
finds every task completed, and moves them to `default.archive.jsonl` (8.5). The owner can print
them with `list --archived` (section 11).

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
| A working-tree folder is deleted by `git clean -fdx`, split across worktrees, and can arrive in a clone | Folded; each measured (15.1) |
| A working-tree folder dirties `git status` | Rejected: `git status --short` prints nothing (15.1) |
| One file per list under an OS lock, temp file and rename | Folded; pattern confirmed at `pairing.rs:282`, `:329`, `:427` |
| Option E, an OpenCode-native build | Rejected: it cited lines 48-107 of `opencode/plugins/todov2-opencode/plugin.ts`, a 46-line file, and an API name that occurs nowhere in it |
| Hosts keep their own list tools beside the tracker | Folded as point 10 |
| A task left in_progress by a dead session needs a lease or process id | Not adopted; point 8 records who changed a task and when |

### Consult round 2 (three store locations)

| Finding | Disposition |
|---|---|
| The git common directory ranks first, the per-user folder second, the working tree last | Folded as point 2 |
| Fall back to the per-user folder when `.git` is not writable | Rejected by the author (5.3); the peer agreed in the agreement round |
| Antigravity states no restriction on where a started process writes | Folded as a statement about its documentation only (15.5) |
| `git gc`, `git prune` and worktree removal leave an unknown folder alone | Was a hypothesis; measured afterwards (15.1) |

### Agreement round (AGREE 11, AMEND 2, DISSENT 0)

| Finding | Disposition |
|---|---|
| Point 9: Qwen and OpenCode already have session-start patterns in this repository; both reviewers had missed them | Folded; the files confirmed (10.2) |
| Point 7: deleting a task leaves ghost blockers when only "blocked by" is stored | Folded |
| Point 7: create should accept blockers, including earlier entries of the same call | Folded |
| Point 7: a flag on create that clears completed tasks | Rejected by the author with a traced case (5.4); the peer agreed |
| OpenCode may start a server in the plugin folder | Rejected for 2.3.0 on a reading of its source (15.4); still in the spike |

### Sign-off rounds (AGREE 3, then AGREE 1)

| Finding | Disposition |
|---|---|
| Archive-on-next-create collides with rejecting a missing blocker | Folded: archive only at MCP server start; a blocker that left the list counts as satisfied |
| One stale pending task stops automatic archiving | Accepted as a known limit; the `archive` subcommand is the remedy |
| A second agent starting while every task is completed archives them out of a running session's view | Below the floor: no open task is lost, nothing is destroyed, no call fails |

### Builder check and panel

Not yet run on this draft. Their findings are recorded here before the owner is asked to approve.

## Stand-downs

- Accepted limit: one stale pending task stops automatic archiving (above).
- Accepted limit: outside a git repository, a subdirectory opens a different list (7.5). This one
  was never put to the peer.
- Below the floor: the running session that loses sight of archived tasks (above); the archive
  holds them and `list --archived` prints them.
- Unverified and carried into the spike: everything in 15.6.
- One clause of point 7 was written after the peer's last sign-off (end of section 6).
