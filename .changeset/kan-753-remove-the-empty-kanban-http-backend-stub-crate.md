---
bump: patch
---

Removed the empty `kanban-http-backend` stub crate (added by KAN-742) from the workspace. Per the sprint-19 decision, the HTTP KanbanBackend implementation will live as a module inside kanban-service (`kanban_service::http_backend`) alongside the JSON and SQLite backends, rather than as a standalone crate, so the placeholder crate is no longer needed. No behaviour change: the crate contained no code.
