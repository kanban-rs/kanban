---
bump: patch
---

`kanban-server`: fixes a write-path scoping hole across the card, column, and
sprint create-or-replace handlers. Each honours a client-supplied id for
idempotent create, but only the column PUT handler already guarded against
that id belonging to a different parent. A `PUT`/`POST` targeting an id that
already exists under a *different* column/board now returns 404 instead of
silently relocating a card or column, or silently editing a sprint that
belongs to a different board.
