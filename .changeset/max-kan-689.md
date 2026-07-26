---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change). The
`kanban-server` binary now actually starts and serves a `/health` endpoint;
previously it was an empty stub that did nothing. This lays the foundation
that the rest of the HTTP API's routes will attach to in follow-up releases.
