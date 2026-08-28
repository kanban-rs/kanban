---
bump: minor
---

server: change events streamed from GET /v1/events now identify which entity changed and how, so a client can invalidate just that board, column, card or sprint instead of its whole cache; an event caused by another process writing the file still carries no entity, meaning everything must be refetched.
