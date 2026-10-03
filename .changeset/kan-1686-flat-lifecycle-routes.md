---
bump: patch
---

server: add `POST /v1/sprints/{id}/{activate,complete,cancel,carry-over}`, flat aliases for the board-scoped sprint lifecycle routes so a client holding only a sprint id can drive the lifecycle in one request. Additive only; the nested routes are unchanged.
