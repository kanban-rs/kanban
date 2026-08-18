---
bump: patch
---

server: `GET /v1/boards/{board_id}/cards` now returns the same `CardResponse` objects as `GET /v1/boards/{board_id}/cards/{id}` instead of a narrower summary shape. List items now carry `board_id`, `prefix` and `description`, and `priority`/`status` are serialized in snake_case (`"in_progress"`) to match every other card endpoint.
