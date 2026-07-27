---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change).
`kanban-server` can now create, replace, update, and delete boards over HTTP
(`POST`/`PUT`/`PATCH`/`DELETE /v1/boards`), not just read them. This is the
first entity to get a complete CRUD surface, establishing the pattern the
remaining entities (columns, cards, sprints) will follow.
