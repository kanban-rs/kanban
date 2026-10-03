---
bump: minor
---

server,backend-http: `GET /health` now reports the server's kanban version, and `HttpBackend` checks it when it opens. A client refuses to open a server older than its own minor release (or one that reports no version) with an error naming both versions and the URL, instead of committing writes it then cannot decode. Upgrade `kanban-server` before upgrading its clients; a client containing this check refuses every server that predates it. Adds `KanbanError::UnsupportedServerVersion` and the shared `kanban_api::HealthResponse` DTO.
