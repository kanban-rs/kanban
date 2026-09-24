---
bump: minor
---

api: adds `MutationResponse<T>` and `DeleteResponse` as new public exports. `MutationResponse<T>` flattens an entity DTO with an `invalidation: InvalidationDto` field, so the body still deserializes as the bare entity while also carrying the mutation's blast radius. `DeleteResponse` carries only `invalidation`, for routes that return no entity body.

server: nine v1 write routes now return the mutation's invalidation: `POST /v1/boards`, `PATCH /v1/boards/{id}`, `POST /v1/boards/{board_id}/columns`, `PATCH /v1/columns/{id}`, `POST /v1/columns/{column_id}/cards`, and `PATCH /v1/cards/{id}` return `MutationResponse<T>`; `DELETE /v1/boards/{id}`, `DELETE /v1/columns/{id}`, and `DELETE /v1/cards/{id}` change from `204 No Content` to `200 OK` with a `DeleteResponse` body. Every other write route (nested board-scoped column/card routes, sprint routes, graph routes) is unchanged.
