---
bump: patch
---

kanban-server now answers 408 Request Timeout for requests that exceed KANBAN_REQUEST_TIMEOUT_SECS (default 30s, 0 to disable); open SSE streams on /v1/events are unaffected.
