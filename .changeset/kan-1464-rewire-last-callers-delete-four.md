---
bump: minor
---

cli,tui,service: rewire the last test call sites off `KanbanContext::snapshot`/`apply_snapshot` and `TuiContext::snapshot`/`apply_snapshot` onto `kanban_service::read_full_snapshot`/`write_full_snapshot`, then delete all four whole-store pass-throughs.
