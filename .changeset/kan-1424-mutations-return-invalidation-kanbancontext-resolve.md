---
bump: minor
---

service: `KanbanContext::execute`, `execute_with`, `execute_with_extra`, `reload`, `replace_backend` and `migrate_sprint_logs` now return the `Invalidation` they computed instead of discarding it, and every mutating `*_impl` inherent method is now `pub` and returns its value paired with the `Invalidation`. `KanbanContext::resolve` is a new thin wrapper over the standalone `kanban_service::resolve` function, so a caller holding a `KanbanContext` can drive a resolve pass without threading the backend through separately.
