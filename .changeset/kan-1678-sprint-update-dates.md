---
bump: patch
---

api: `PATCH /v1/sprints/{id}` (nested and flat routes) now accepts `start_date` and `end_date` as `Patch<DateTime<Utc>>`, so callers can set or clear a sprint's dates instead of only leaving them unchanged. `status` and `name_index` stay off the wire.
