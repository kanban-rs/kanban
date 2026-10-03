---
bump: patch
---

service,backend-http: `move_card_impl`, `assign_card_to_sprint_impl` and `unassign_card_from_sprint_impl` now divert to `KanbanBackend::remote_card_writes()` as their first statement, before any local read (column count, sprint batch delegation). Over `HttpBackend` each op is now one request and zero client reads, returning the server's `Invalidation` verbatim.
