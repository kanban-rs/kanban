---
bump: minor
---

`kanban-server` now exposes read routes for cards:

- `GET /v1/boards/{board_id}/cards` — list a board's cards, optionally filtered
  by `column_id`, `sprint_id`, and a new `archived` query parameter
  (`live_only` [default], `archived_only`, or `include`). An unknown
  `board_id` returns an empty array rather than an error, matching the
  existing board/column list endpoints.
- `GET /v1/boards/{board_id}/cards/{id}` — fetch a single card. Returns 404 if
  the card doesn't exist or belongs to a different board than the one in the
  path.

Cards returned with `archived=include` or `archived=archived_only` carry a
stamped `archived_at` timestamp so clients can distinguish archived cards from
live ones without a separate lookup.
