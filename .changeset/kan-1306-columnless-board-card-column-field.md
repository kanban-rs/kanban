---
bump: patch
---

tui: The create-card dialog now shows a "Column:" field naming exactly where the new card will land. On a board that already has columns the field is disabled and greyed, filled in from the destination column and cannot be edited. On a board with no columns yet, the field is editable, prefilled with the template column name, and its value (falling back to the template name if left blank) becomes the name of the column the TUI creates alongside the card. That column create now shares the card create's undo transaction, so a single undo removes both, and the invented column carries the template's default status instead of none. The CLI's `kanban card create` still requires `--column` explicitly, since a CLI user can simply chain a second `column create` command.
