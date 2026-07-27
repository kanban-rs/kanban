---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change). Errors
raised inside `kanban-server` now consistently convert to the right HTTP
status code and a flat JSON error body, instead of each future route having
to map that itself. This is the last piece the API's routes need before they
can start landing.
