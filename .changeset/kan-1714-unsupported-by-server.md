---
bump: minor
---

backend-http,domain,api: a write the kanban server has no route for (404 or 405 without an error envelope) now fails with KanbanError::UnsupportedByServer, naming the route and asking for a server upgrade, instead of "internal error: HTTP 404 Not Found". is_unsupported() is true for it. Adds KanbanError::UnsupportedByServer and KanbanError::unsupported_by_server.
