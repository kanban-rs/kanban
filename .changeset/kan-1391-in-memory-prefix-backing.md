---
bump: patch
---

backend-memory: reject a card whose prefix has no backing `prefixes` row, matching the JSON and SQLite backends. Previously `InMemoryStore` accepted such a card, which made the in-memory backend non-substitutable for the durable ones and let a green in-memory test pass despite a state the real backends would reject. The empty prefix stays exempt on all three backends.
