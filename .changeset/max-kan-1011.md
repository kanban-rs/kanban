---
bump: patch
---

`kanban-server` now exposes flat, non-board-scoped routes for columns and cards: `GET/PATCH/DELETE /v1/columns/{id}` and `GET/PATCH/DELETE /v1/cards/{id}`. These behave identically to the existing board-scoped routes (`/v1/boards/{board_id}/columns/{id}`, `/v1/boards/{board_id}/cards/{id}`) but don't require the caller to know which board owns the entity up front. The existing board-scoped routes are unchanged and continue to work exactly as before.
