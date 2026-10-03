---
bump: patch
---

server: the four flat card/column write routes (`PATCH`/`DELETE /v1/cards/{id}` and `PATCH`/`DELETE /v1/columns/{id}`) now resolve existence before evaluating `If-Match`. An unknown id now answers `404` regardless of whether an `If-Match` header is present, matching RFC 9110 §13.2.1 and the nested board-scoped routes for the same entities.
