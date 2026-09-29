---
bump: patch
---

service: `archive_cards`, `move_cards`, `assign_cards_to_sprint` and `update_cards` divert to `RemoteBatchWrites` as their first statement when a backend declares it, returning the server's count and invalidation verbatim (or the first failure as a validation error when every id fails). Falls back to the existing local command-execute-then-log path otherwise. Additive: `MockBackend` gains an optional batch-writes field alongside its existing per-family mocks.
