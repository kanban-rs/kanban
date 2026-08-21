---
bump: minor
---

BREAKING: `GET /v1/boards/{id}/columns` and `GET /v1/boards/{id}/cards` now return 404 when the board does not exist, instead of an empty page. This matches the sprints route and lets a client tell a deleted board from an empty one.
