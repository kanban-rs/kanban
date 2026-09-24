---
bump: patch
---

kanban-server exposes POST /v1/boards/{id}/archive, POST /v1/boards/{id}/restore and GET /v1/archived-boards so HTTP clients can remove a board reversibly instead of only permanently deleting it
