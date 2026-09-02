---
bump: patch
---

service: rewire kanban-service's cross-backend contract tests and integration tests off `DataStore::snapshot`/`apply_snapshot` onto `read_full_snapshot`/`write_full_snapshot`, and onto scoped per-collection reads where that is what the test already asserts. Test-only change; `KanbanContext::snapshot`/`apply_snapshot` and the `prefix.rs` contract are untouched.
