---
bump: minor
---

server: add flat `POST /v1/cards/{id}/move`, `/assign-sprint` and `/unassign-sprint` routes that run the service's `move_card_impl`, `assign_card_to_sprint_impl` and `unassign_card_from_sprint_impl` and return the mutation's invalidation, so a remote client can move a card (server-side append position and status chaining) and bind or unbind a sprint (sprint log written server-side) without the PATCH shortcut that skips both.
