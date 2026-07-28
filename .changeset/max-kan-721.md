---
bump: minor
---

`kanban-server` now supports creating, replacing, updating, and deleting cards
over HTTP:

- `POST /v1/columns/{column_id}/cards` — create a card in the given column.
- `PUT /v1/columns/{column_id}/cards/{id}` — idempotent create-or-replace for
  a card at a client-supplied id.
- `PATCH /v1/boards/{board_id}/cards/{id}` — partial update (JSON Merge
  Patch). Moving a card to a different column enforces that column's WIP
  limit, returning a 409 if it's full.
- `DELETE /v1/boards/{board_id}/cards/{id}` — delete a card.

Every mutation is durably persisted before the response is returned, and
broadcasts a change event to connected clients. A request for a card that
doesn't exist, or that belongs to a different board than the one in the URL,
returns 404.
