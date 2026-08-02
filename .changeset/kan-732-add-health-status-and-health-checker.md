---
bump: patch
---

Added a `HealthStatus` enum and `HealthChecker` trait to `kanban-core`, and an optional `health_checker()` hook on the `KanbanBackend` trait. This is an internal infrastructure addition with no user-visible behaviour change. It will be used by the upcoming HTTP collaborative backend to power the `GET /health` endpoint, letting the server report whether its underlying storage backend is reachable and healthy.
