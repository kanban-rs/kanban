---
bump: patch
---

kanban-server exits cleanly on SIGTERM and ctrl-c, draining in-flight requests and open SSE streams within a bounded window
