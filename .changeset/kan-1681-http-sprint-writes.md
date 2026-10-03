---
bump: patch
---

backend-http: `HttpBackend` implements `RemoteSprintWrites` (`create_sprint`, `update_sprint`, `delete_sprint`) over `POST /v1/boards/{board_id}/sprints`, `PATCH /v1/sprints/{id}` and `DELETE /v1/sprints/{id}`, and `remote_sprint_writes()` now returns `Some(self)`. `update_sprint` declines `name_index` and `status` as server-managed fields. `get_sprint` now shares the board-pool lookup with the new `sprint_with_pool` helper instead of duplicating it.
