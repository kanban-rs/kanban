---
bump: patch
---

api: a sprint's wire response is now built from the sprint plus its already-resolved display name, so reading one sprint no longer structurally requires loading its board; the board lookup moved once into the service layer (`kanban_service::resolve_sprint_name`/`resolve_sprint_names`), and the server, CLI, and MCP sprint-list paths still read the owning board only once.
