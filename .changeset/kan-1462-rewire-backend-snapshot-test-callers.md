---
bump: minor
---

kanban-persistence-json, kanban-persistence-sqlite, kanban-persistence: rewire the test call sites off `DataStore::snapshot`/`apply_snapshot` onto their backend-specific replacements (`snapshot_async`/`apply_snapshot_async`, `read_full_snapshot`/`write_full_snapshot`, `snapshot_impl`), and widen `SqliteStore::snapshot_async`/`apply_snapshot_async` from `pub(crate)` to `pub` so a separate crate's tests can call them directly.
