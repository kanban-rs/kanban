---
bump: minor
---

Boards now have configurable completion columns instead of guessing that the last column means "done". Previously, marking a card done always moved it to the board's last column, and moving a card into the real Done column could silently reset its status back to todo — on any board whose done column was not physically last, the two rules fought each other forever.

Each board now carries an ordered list of completion columns. The first entry is where a card goes when its status is set to done; membership in the list is what marks a card complete, so boards with several terminal columns (for example Done plus Won't Do) work correctly and moving a card between them leaves its status alone. An empty list turns the status/column coupling off entirely: status changes never move a card, and moves never change a status.

Configure it from any surface — CLI (`board update <board> --completion-columns Done,Decision`, names or UUIDs, empty string disables), HTTP (`PATCH /v1/boards/:id` with `completion_column_ids`), or MCP (`tool_update_board`). Every surface validates the list, so a deleted column, another board's column, or a duplicate is rejected. Boards created in the TUI come pre-configured (the default template sets Complete), so a fresh board needs no setup; bare CLI/API boards start with the coupling off.

If marking cards done has been filing them under the wrong column (any board whose done column is not physically last), fix it with one command after upgrading: `kanban <file> board update <board> --completion-columns <your done column>`.

Existing files upgrade automatically with no behaviour change: the JSON format moves to V12 and the SQLite schema to version 6, and every board's list is backfilled to what the old last-column rule resolved, with a durable backup (`.v11.backup` / `.v5.backup`) written beside the file. The one deliberate difference: boards with duplicate column positions now resolve their backfilled completion column deterministically instead of depending on storage order.

Removed: the old single `completion_column_id` field is gone from the HTTP API (requests and responses) and from stored data. It was write-only in practice — nothing but a rarely-used HTTP PATCH could set it — and its value is carried into the new list by the migration. Files upgraded to the new formats cannot be opened by older versions of the binary; restore the written backup to roll back.
