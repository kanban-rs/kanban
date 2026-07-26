---
bump: minor
---

Creating a new board no longer accepts a completion-column setting, since a
brand-new board never has any columns yet for it to point at — that field
was always silently invalid there and is now rejected instead of being
accepted and ignored. To set which column marks a card as complete, create
the board's columns first, then set it via the board-update API.

This applies to MCP's board-creation tool and the underlying create API; the
separate "replace an existing board" API is unaffected and can still set the
completion column, since a board being replaced may already have columns.
