---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change). Adds a
real-socket integration test harness for `kanban-server` — every existing
route test drove the router in-process without an actual network socket;
this proves the server also works end-to-end as a real running process,
and is the fixture future work on a collaborative HTTP backend will build on.
