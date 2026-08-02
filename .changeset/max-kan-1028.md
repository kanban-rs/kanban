---
bump: patch
---

Internal test reorganisation with no user-visible effect: `kanban-domain`'s command tests move from inline unit-test modules into integration tests. A test that needs a concrete storage implementation to run is an integration test, so this puts them where they belong, and it lets the in-memory store be extracted into its own crate later. Behaviour is unchanged. Six tests that exercised private validation helpers directly were removed as duplicates of existing tests covering the same scenarios through the public command API, and two scenarios that were only covered by such a test are now covered through the public API instead.
