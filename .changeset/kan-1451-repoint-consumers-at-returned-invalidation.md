---
bump: minor
---

domain,service,mcp,tui: `Invalidation` is now `#[must_use]`, and `KanbanContext::undo`/`redo` return `KanbanResult<Option<Invalidation>>` instead of `KanbanResult<bool>`, carrying the invalidation the reversed or replayed batch implies rather than discarding it. `KanbanContext::last_invalidation` and its backing field are removed; every caller now reads the value returned from `execute`, `execute_with`, `execute_with_extra`, `undo`, or `redo` directly. `McpContext::undo`/`redo` forward the same `Option<Invalidation>` return type. `TuiContext::undo`/`redo` are unchanged and still return `bool`, since kanban-tui has no consumer for the invalidation yet.
