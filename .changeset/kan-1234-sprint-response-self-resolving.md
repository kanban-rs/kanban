---
bump: patch
---

api: `SprintResponse::new(sprint, name)` builds the wire projection from an already-resolved name instead of the owning `Board`. server: the sprint routes now resolve a sprint's name through `kanban-service` rather than fetching the board themselves.
