---
bump: patch
---

kanban-server exposes POST /v1/cards/{id}/archive and POST /v1/cards/{id}/restore so HTTP clients can remove a card reversibly instead of only permanently deleting it
