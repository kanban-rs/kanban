---
bump: patch
---

web: `kanban-web`'s home page now derives column ordering and per-column card filtering/sorting from a `kanban_view::model::Model` built off boards/columns/cards, instead of hand-rolled iteration, so it shares the same view logic `kanban-tui` uses. Reads are still per-board/per-column calls (not `ctx.snapshot()`, which `HttpBackend` doesn't implement), so remote reads via `KANBAN_SERVER_URL` keep working. No visible output change.
