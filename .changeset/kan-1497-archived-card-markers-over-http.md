---
bump: minor
---

api,server,backend-http: add `ArchivedCardResponse` and a
`GET /v1/boards/{board_id}/archived-cards` route, then wire
`HttpBackend::list_archived_cards_by_board` to call it so the remote
backend can serve the board-scoped archived-card tier instead of
returning `Unsupported`.
