---
bump: minor
---

Boards now have configurable completion columns instead of guessing that the last column means "done". Previously, marking a card done always moved it to the board's last column, and moving a card into the real Done column could silently reset its status back to todo — on any board whose done column was not physically last, the two rules fought each other forever.

Each board now carries an ordered list of completion columns. The first entry is where a card goes when its status is set to done; membership in the list is what marks a card complete, so boards with several terminal columns (for example Done plus Won't Do) work correctly and moving a card between them leaves its status alone. An empty list turns the status/column coupling off entirely: status changes never move a card, and moves never change a status.

Configure it from any surface:

- CLI: `kanban <file> board update <board> --completion-columns Done,Decision` (names or UUIDs, most-primary first; pass an empty string to disable the coupling)
- HTTP: `PATCH /v1/boards/:id` with `completion_column_ids` (null or `[]` disables; the field is validated so a column of another board or a deleted one is rejected)
- MCP: `tool_update_board` accepts `completion_column_ids` as column names or UUIDs

Boards created in the TUI come pre-configured: the default template (TODO, Doing, Complete) sets Complete as the completion column at creation, so a fresh board's status/column sync works with no setup step. Boards created bare (CLI or API, no columns) start with the coupling off until configured.

If marking cards done has been filing them under the wrong column (any board whose done column is not physically last), fix it with one command after upgrading: `kanban <file> board update <board> --completion-columns <your done column>`.

Existing files upgrade automatically with no behaviour change: the JSON format moves to V12 and the SQLite schema to version 6, and every board's list is backfilled to what the old last-column rule resolved, with a durable backup (`.v11.backup` / `.v5.backup`) written beside the file. The one deliberate difference: boards with duplicate column positions now resolve their backfilled completion column deterministically instead of depending on storage order.

Removed: the old single `completion_column_id` field is gone from the HTTP API (requests and responses) and from stored data. It was write-only in practice — nothing but a rarely-used HTTP PATCH could set it — and its value is carried into the new list by the migration. Files upgraded to the new formats cannot be opened by older versions of the binary; restore the written backup to roll back.
