---
bump: patch
---

server: `PATCH`/`DELETE /v1/boards/{id}` resolve existence before evaluating `If-Match`, so an unknown board id answers `404` instead of `412` regardless of the header (RFC 9110 §13.2.1).
