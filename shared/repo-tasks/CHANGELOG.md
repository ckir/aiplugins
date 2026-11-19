# Changelog

All notable changes to the `repo-tasks` engine crate are documented here.

## [Unreleased]

### Added

- First version of the engine: a shared, file-backed task list for one repository, served over
  MCP (`task_create`, `task_get`, `task_update`, `task_list`) and through the `serve`,
  `reminder`, `list` and `archive` subcommands.
- Store resolution inside git (the git common directory) and outside git (per-user data
  directory), with locking, atomic writes and an archive of completed tasks.
- Windows tests for path canonicalisation and for a reader and a writer running together. Their
  results are not measured yet (see the README).
