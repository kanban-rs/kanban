---
bump: patch
---

Internal test reorganisation with no user-visible effect: `kanban-domain`'s command tests move from inline unit-test modules into integration tests. A test that needs a concrete storage implementation to run is an integration test, so this puts them where they belong and lets the in-memory store be extracted into its own crate later. All existing behaviour and test coverage are unchanged.
