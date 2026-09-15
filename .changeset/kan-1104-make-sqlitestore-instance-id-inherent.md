---
bump: minor
---

persistence-sqlite: add an inherent `SqliteStore::instance_id` method so callers no longer need `PersistenceStore` in scope to read it, and switch `SqliteBackend`'s own lookup to use it.
