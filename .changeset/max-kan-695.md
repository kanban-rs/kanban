---
bump: minor
---

`kanban-server` now supports batch command execution: `POST /v1/commands`
accepts a batch of commands and runs them as a single atomic transaction,
using the same transactional engine the desktop app and CLI use locally.

- If every command in the batch succeeds, the response is
  `{ "executed": <count> }` and one change event is broadcast to connected
  clients for the whole batch.
- If any command in the batch fails, the entire batch is rolled back — no
  partial writes — and nothing is broadcast.

This is the primary write path for programmatic/multi-step clients; the
existing per-entity REST routes (create/update/delete a single board, column,
or card) remain available as convenience wrappers for simpler cases.
