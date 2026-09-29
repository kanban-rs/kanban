---
bump: patch
---

server: add `POST /v1/sprints/{id}/{activate,complete,cancel,carry-over}`, flat aliases for the board-scoped sprint lifecycle routes so a client holding only a sprint id can drive the lifecycle in one request. The flat routes run with no board-scope check at all — a flat carry-over accepts a `to_sprint_id` on a different board, matching what a local (non-HTTP) carry-over does, while the board-scoped route still 404s on a board mismatch. Additive only; the nested routes are unchanged.
