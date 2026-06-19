---
bump: patch
---

Internal infrastructure change with no user-visible behaviour difference. Adds the `api` module to kanban-service (`kanban_service::api`) holding the shared HTTP wire types for the upcoming collaborative backend: ApiError (the HTTP error envelope) and ChangeEventFrame (the SSE frame carrying writer identity, correlation ID, and timestamp). These form the contract between the server and all clients. They live as a module in kanban-service rather than a standalone crate so the server and every KanbanBackend transport share one definition without a separate crate boundary.
