---
bump: patch
---

api,server: reject a legacy `completion_column_ids` key on board writes (`POST /v1/boards`, `PATCH /v1/boards/:id`, `PUT /v1/boards/:id`) with a 422 naming `default_status` as the replacement, instead of silently ignoring it. `completion_column_ids` never shipped in a release, so no existing client is affected by this change; it guards against the key some readers of unreleased develop history might plausibly try. Set `default_status` on a column via the column endpoints (`POST /v1/boards/:board_id/columns`, `PATCH /v1/columns/:id`, `PUT /v1/boards/:board_id/columns/:id`) and read it back on `ColumnResponse.default_status`. Also corrects the `CreateBoardRequest` doc comment, which pointed integrators at `update_board`, a request that has no such field and never did.
