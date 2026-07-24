---
bump: patch
---

Fixed a data-integrity bug where an archived card could permanently lose its
board association, making it invisible to that board's archived-cards list
and unreachable by the cleanup that runs when a board is deleted. This
happened in an edge case: archiving a card, then deleting its now-empty
column (allowed since archived cards don't block column deletion), then
archiving that same card again (e.g. a retried save or an undo/redo replay) —
the second archive used to silently overwrite the card's board with "unknown"
instead of leaving the already-correct value alone.

Under the hood, every card now carries its own durable reference to its
board, kept up to date whenever a card moves to a different column (including
moving it to a column on a different board, which continues to work exactly
as before). This removes the need to look up a card's board through its
column at archive time, so the board reference can no longer go stale even
if the column is later removed.

Existing boards, cards, and databases are migrated automatically the next
time they're opened — no action needed.
