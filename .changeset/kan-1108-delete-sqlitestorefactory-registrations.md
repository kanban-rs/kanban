---
bump: minor
---

persistence-sqlite: removes the public `SqliteStoreFactory` struct and its
`StoreFactory` impl, since `StoreManager::make_backend` and `detect_backend`
already resolve SQLite locators through the `KanbanBackendRegistry` alone.
service: `StoreManager::detect_backend` now delegates its fallback sniff to
the backend registry instead of a hardcoded `#[cfg(feature = "sqlite")]`
magic-byte check, so it also recognises a not-yet-created `.db` path.
mcp: adds `McpServer::register_backend_only` for registering a backend
factory without a paired `StoreFactory`.
