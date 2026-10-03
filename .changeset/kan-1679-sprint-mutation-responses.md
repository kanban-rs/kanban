---
bump: minor
---

server: `POST /v1/boards/{board_id}/sprints` and `PATCH /v1/sprints/{id}` now return their body wrapped in a `MutationResponse` carrying the mutation's `invalidation` (the entity fields stay readable as the bare `SprintResponse`), and `DELETE /v1/sprints/{id}` now returns `200 OK` with a `DeleteResponse` body instead of `204 No Content`. The nested `PUT`/`PATCH`/`DELETE /v1/boards/{board_id}/sprints/{id}` routes are unchanged (`DELETE` still answers `204`).
