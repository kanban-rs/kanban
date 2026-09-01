---
bump: patch
---

kanban-tui: column and board delete/move/create handlers now decline with an
error banner when the board-scoped column or sprint tier is not loaded,
instead of silently acting on an empty collection derived from the flat
model tier. `App::load_snapshot` now repopulates the board-scoped column and
sprint tiers from the flat snapshot it loads, so the declining handlers stay
functional on every real reload instead of always taking the decline branch.
