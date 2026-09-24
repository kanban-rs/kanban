---
bump: minor
---

persistence-sqlite: removes `impl PersistenceStore for SqliteStore` and the
`snapshot_async`/`apply_snapshot_async` whole-store pair, along with the
`list_archived_boards_async`, `list_prefixes_async`, and
`write_prefix_with_conn` helpers that existed only to support them.
`SqliteStore` gains a new inherent `close` method; `SqliteBackend::close`,
the sole production caller, now goes through it directly instead of the
trait. SQLite is now reachable only as a `KanbanBackend`; nothing in the
workspace can ask it for its whole contents as a single storage-format
value. `kanban-persistence` stays a dependency of `kanban-persistence-sqlite`
for `PersistenceMetadata`.
