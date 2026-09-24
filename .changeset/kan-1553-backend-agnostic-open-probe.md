---
bump: minor
---

kanban_context::open now probes backend liveness instead of the local command log, so opening a context over a remote HttpBackend succeeds instead of failing on batch_count
