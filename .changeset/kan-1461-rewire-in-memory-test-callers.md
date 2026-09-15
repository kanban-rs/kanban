---
bump: patch
---

backend-memory: rewire the 14 test-only callers of `InMemoryStore::snapshot`/`apply_snapshot` onto the already-public inherent methods `snapshot_impl`/`apply_snapshot_impl`, so the tests no longer depend on the `DataStore` trait methods slated for removal.
