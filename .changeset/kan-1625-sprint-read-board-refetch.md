---
bump: patch
---

backend-http: `list_all_sprints` no longer refetches a live board's body per id for its `sprint_names`; live boards' bodies are already in hand from the single `GET /v1/boards` call. Archived boards keep their per-id refetch, since their bodies aren't returned by that route. Same sprints, resolved names and order as before, fewer requests.
