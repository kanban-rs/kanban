---
bump: patch
---

tui: card_handlers.rs no longer collapses a NotLoaded columns or sprints
tier to an empty collection when creating, moving, restoring, or assigning
a card, or when opening the manage-children picker. An unloaded tier now
sets an error banner and leaves the prior state untouched instead of
silently acting as if the board had zero columns or sprints.
