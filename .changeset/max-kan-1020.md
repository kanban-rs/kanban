---
bump: patch
---

Internal restructuring with no user-visible effect: the backend abstraction (`KanbanBackend`, `RemoteWrites`) and the HTTP wire types now live in their own `kanban-backend` and `kanban-api` crates instead of inside `kanban-service`. This lets storage and transport implementations be plugged in independently of the service layer, and is groundwork for connecting the CLI, TUI, and MCP server to a remote `kanban-server`. All existing behaviour, on-disk formats, and APIs are unchanged.
