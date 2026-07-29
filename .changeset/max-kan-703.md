---
bump: patch
---

Internal test infrastructure change only, no user-visible effect: `kanban-server`'s in-process test harness (`TestServer`, used to spin up a real server over a real socket for integration tests) moved from a test-only file into a `test-helpers`-feature-gated library module, so other crates in the workspace can reuse it in their own tests. Not included in the production `kanban-server` binary.
