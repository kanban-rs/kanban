---
bump: patch
---

backend-http: `HttpBackend` implements `RemoteBatchWrites`, sending
`archive_cards`, `move_cards`, `assign_cards_to_sprint` and `update_cards`
to the existing `/v1/cards/batch/*` routes and mapping the response into
`RemoteBatchOutcome`. Additive; no route or DTO changes.
