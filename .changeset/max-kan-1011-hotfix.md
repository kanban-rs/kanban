---
bump: patch
---

Fixes a `kanban-server` build break introduced by two independently-developed changes landing back to back: the flat-route integration tests (`tests/flat_routes.rs`) still referenced a test helper module that had been relocated in a parallel change, so `kanban-server` failed to compile with the `test-helpers` feature enabled. No functional change — the test file now imports from the correct location.
