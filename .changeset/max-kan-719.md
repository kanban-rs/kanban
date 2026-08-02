---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). Adds column write routes to `kanban-server`: `POST`/`PUT`/`PATCH`/`DELETE` and a dedicated reorder endpoint under `/v1/boards/{board_id}/columns`. `PUT` is a true full replace — it can move a column's position, not just rename it. Every write route now verifies the target column actually belongs to the board named in the URL, matching the read routes.
