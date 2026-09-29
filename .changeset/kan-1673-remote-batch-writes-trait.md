---
bump: minor
---

backend: `RemoteBatchWrites` gains four required methods (`archive_cards`, `move_cards`, `assign_cards_to_sprint`, `update_cards`) and a new `RemoteBatchOutcome` struct carrying per-id success/failure results. No in-tree type implements the trait yet, so nothing breaks, but any external implementor must add these methods.
