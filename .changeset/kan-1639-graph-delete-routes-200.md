---
bump: minor
---

server: the three graph edge DELETE routes (`DELETE /v1/cards/{id}/children/{child_id}`, `DELETE /v1/cards/{id}/blocks/{blocked_id}`, `DELETE /v1/cards/{id}/related/{other_id}`) now return `200 OK` with a `DeleteResponse` body carrying the mutation's `invalidation`, instead of `204 No Content` with an empty body. This matches the other five graph write routes and the existing flat card delete, and is a breaking change for any client that expected `204`.
