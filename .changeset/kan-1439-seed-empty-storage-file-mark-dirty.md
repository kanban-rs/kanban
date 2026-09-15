---
bump: patch
---

cli: seed a freshly created storage file's dirty flag via the `KanbanBackend::mark_dirty` primitive instead of a `needs_save_worker`-guarded `apply_snapshot(Snapshot::new())` call, so every backend gets the same unconditional write on `kanban init`.
