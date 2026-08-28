---
bump: patch
---

backend-memory: reject a card whose prefix has no backing `prefixes` row, on both `upsert_card` and `apply_snapshot`, matching the JSON and SQLite backends. Previously `InMemoryStore` accepted such a card on either path, which made the in-memory backend non-substitutable for the durable ones and let a green in-memory test pass despite a state the real backends would reject. The empty prefix stays exempt on all three backends.

Because `JsonDataStore::upsert_card` delegates to the inner `InMemoryStore`, the JSON backend's own behaviour changes too, not just its test double: it now rejects an unbacked prefix on the individual write, the same as SQLite's per-statement foreign key, rather than deferring the check to batch flush. A transaction that writes more than one unbacked card now reports whichever one is written first, which may not be the same offender the old batch-level check would have named. `JsonDataStore::ensure_batch_namespaces_backed` still runs at commit but can no longer find anything to reject, since every card recorded into the batch has already passed the eager check.
