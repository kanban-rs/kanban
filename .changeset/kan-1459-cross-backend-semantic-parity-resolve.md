---
bump: minor
---

service: add fifteen cross-backend contract tests under `test_helpers::contract::cache` pinning `resolve`'s semantic parity, not just its `LoadState` mapping, across the in-memory, JSON and SQLite backends. Covers scoped-vs-list read agreement, archival/reversibility identity (including a full `assert_card_eq` round trip through delete/undo and archive/restore), the dependency graph tier (including edges with an archived endpoint), reopen-after-flush freshness, and invalidation scoping. No production code changed.
