---
bump: patch
---

Board-scoped archived-card listing now reads the first-class `board_id` field
instead of inferring the board from each archived card's (possibly stale)
original column, so an archived card whose original column was later deleted
still appears in its board's archived list. `DeleteColumn` no longer blocks on
archived cards: an archived card's `original_column_id` is historical, not a
live foreign key, so a column that holds only archived cards can be deleted. The
board-delete cascade now gathers archived cards by `board_id` and removes both
their records and their dependency-graph edges via a new `DeleteArchivedCards`
cascade command, closing an orphan leak that occurred when a card's original
column had been deleted after archival. Importing a board now backfills
`board_id` on archived cards that predate the first-class field (reconstructing
it from the archived card's original column), so a legacy export's archived
cards remain visible in board-scoped listing and are not leaked on board delete.
The board-delete cascade also no longer short-circuits a board that has sprints
but no columns or archived cards, so deleting such a board removes its sprints
and undo restores them. The now-unused `DeleteArchivedCardsByColumns` cascade
command and the `list_archived_cards_by_columns` store method have been removed.
