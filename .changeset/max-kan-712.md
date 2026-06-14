---
bump: patch
---

Internal infrastructure change with no user-visible behaviour difference. Introduces the kanban-api crate containing the shared wire types for the upcoming HTTP collaborative backend: ApiError (the HTTP error envelope) and ChangeEventFrame (the WebSocket push frame carrying writer identity, correlation ID, and timestamp). These types form the contract between the server and all clients.
