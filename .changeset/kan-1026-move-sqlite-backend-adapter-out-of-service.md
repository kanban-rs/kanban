---
bump: patch
---

Internal groundwork with no user-visible effect: the SQLite backend adapter now lives in `kanban-persistence-sqlite`, alongside the store it wraps, instead of `kanban-service`. The `KanbanBackendFactory` trait's `scheme()` method is renamed to `name()` to match the registry's dispatch-by-backend-name semantics. `kanban-service` still constructs the SQLite backend directly; behaviour, storage formats, and commands are unchanged.
