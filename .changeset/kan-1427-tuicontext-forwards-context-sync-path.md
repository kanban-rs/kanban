---
bump: minor
---

kanban-tui: `TuiContext` gains two thin forwarding methods, `sync` and `sync_invalidated`, delegating to the identically named `KanbanContext` methods. This gives the TUI its only production path to the resolve-and-apply-and-resync seam, mirroring the pass-through pattern already used by `kanban-cli` and `kanban-mcp`. Both methods take `&self` and never touch the save coordinator, since a read must never queue a save.
