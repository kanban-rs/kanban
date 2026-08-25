---
bump: patch
---

backend-memory: reject a card whose prefix has no backing `prefixes` row, on both `upsert_card` and `apply_snapshot`, matching the JSON and SQLite backends. Previously `InMemoryStore` accepted such a card on either path, which made the in-memory backend non-substitutable for the durable ones and let a green in-memory test pass despite a state the real backends would reject. The empty prefix stays exempt on all three backends.

Because `JsonDataStore::upsert_card` delegates to the inner `InMemoryStore`, the JSON backend's enforcement moves from deferred-to-end-of-batch to eager as a side effect, which brings it closer to SQLite's per-statement foreign key. Its batch-level check becomes unreachable in that path.
