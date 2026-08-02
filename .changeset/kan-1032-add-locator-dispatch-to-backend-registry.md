---
bump: patch
---

Internal groundwork with no user-visible effect: `KanbanBackendFactory` gains a `matches_locator` method (default: never matches by content) and `KanbanBackendRegistry` gains `for_locator`, which reads up to the first 32 bytes of a locator and asks each registered factory in registration order whether it should handle it. This mirrors the existing `StoreRegistry::detect_backend` dispatch pattern one layer down. `SqliteBackendFactory` now matches SQLite magic bytes or a `.sqlite`/`.sqlite3`/`.db` extension on a new file; `JsonBackendFactory` is the catch-all. Nothing calls `for_locator` yet, so behaviour, storage formats, and commands are unchanged.
