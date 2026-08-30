---
bump: minor
---

domain,service: add a resolve/invalidate parity contract harness that pins `resolve`'s `LoadState` mapping (absent card/column/sprint resolves to `Missing`, a backend read or list error resolves to `Failed`, `Missing` is terminal, `Failed` is retried) across the in-memory, JSON, and SQLite backends, and document the archival semantics of `DataStore::get_card` and `DataStore::list_all_cards` on the trait itself.
