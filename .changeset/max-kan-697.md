---
bump: patch
---

Internal groundwork for the upcoming HTTP collaborative backend: a new `kanban-backend-http` crate now exists, scaffolding `HttpBackend` (a `KanbanBackend` implementation that will eventually talk to a remote `kanban-server` over HTTP). This release has no user-visible effect — the crate isn't wired into any consumer yet, and every read/write method is an explicit stub returning "not supported." `kanban-service` itself gained no new dependencies from this work.
