---
bump: patch
---

Internal persistence change with no user-visible behaviour. The SQLite backend
schema advances 2 -> 3: `archived_cards` gains a first-class `board_id TEXT NOT
NULL` column (indexed via `idx_archived_cards_board_id`) and the 2 -> 3
migration backfills it from each row's `original_column_id` -> `columns.board_id`
mapping, falling back to a nil board id when the original column no longer
resolves (unrecoverable, tolerated rather than failing the open). The migration
runs before the schema is (re)applied, is idempotent on an already-migrated
database, and preserves archived sprint logs. The `cards.column_id -> columns(id)`
foreign key is dropped (the value stays `TEXT NOT NULL` and intact) so an
archived card survives deletion of its original column; live-card cleanup on
column delete is already explicit via the command tier, so the cascade was
redundant. `list_archived_cards_by_board` now overrides with a direct `WHERE
board_id = ?` query, and the archived-card exclusion filters switch to `NOT
EXISTS`. The board-scoping consumer that reads the backfilled field is deferred
to a later slice.
