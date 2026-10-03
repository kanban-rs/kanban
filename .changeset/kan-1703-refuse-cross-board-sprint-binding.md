---
bump: minor
---

domain,service,server: a card can no longer be bound to a sprint on a different board. Assigning (single, batch, detailed batch), carrying over, and setting `sprint_id` through a card update now fail with `SprintBoardMismatch` (`422 SPRINT_BOARD_MISMATCH` over HTTP) on every backend, so the flat `POST /v1/sprints/{id}/carry-over` refuses a target sprint on another board instead of accepting it; the nested route still answers 404. The detailed batch assign fails only the cards on another board. Undoing a sprint delete or a card update now restores the card's sprint binding and sprint log exactly. Existing cross-board bindings are not migrated and keep reading; import still restores them. The `SprintBoardMismatch` message now reads "...but the card is on board ..." instead of "...but card is being created on board ...". Breaking for callers that relied on cross-board binding.
