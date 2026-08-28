---
bump: minor
---

Completion no longer guesses that a board's last column means "done". Previously, marking a card done always moved it to the board's last column, and moving a card into the real Done column could silently reset its status back to todo — on any board whose done column was not physically last, the two rules fought each other forever.

This work first introduced a board-level list of completion columns, but that design was superseded within this same release (see KAN-1163/KAN-1168): v0.9.0 ships completion derived from each column's `default_status` instead. A card marked done files under the board's first status=done column; moving a card into a column applies that column's default status; a board with no status=done column has the coupling off entirely. Configure it per column from any surface — CLI (`column create`/`column update` with `--default-status done`, `--clear-default-status` to remove), HTTP (`default_status` on the column endpoints: `POST /v1/boards/:board_id/columns`, `PATCH /v1/columns/:id`, `PUT /v1/boards/:board_id/columns/:id`), or MCP (`tool_update_column`). There is no `board update --completion-columns` flag in the released binary.

Existing files upgrade automatically with no behaviour change: this release moves the JSON format to V18 and the SQLite schema to version 13, backfilling every board's completion column to what the old last-column rule resolved and deriving `default_status` from it, with a durable backup (`.v{N}.backup`) written beside the file. Boards with duplicate column positions now resolve their backfilled completion column deterministically instead of depending on storage order.

Removed: the old single `completion_column_id` field and the interim `completion_column_ids` list are gone from the HTTP API (requests and responses) and from stored data; their value is carried into the per-column `default_status` by the migration. Files upgraded to the new formats cannot be opened by older versions of the binary; restore the written backup to roll back.
