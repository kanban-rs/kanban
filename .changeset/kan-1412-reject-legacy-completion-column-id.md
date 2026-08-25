---
bump: patch
---

api,server: reject a legacy singular `completion_column_id` key on `PATCH /v1/boards/:id` and `PUT /v1/boards/:id` with a 422 naming `default_status` as the replacement, instead of silently dropping it. This is the key every released 0.8.1 client actually sends; the previous guard checked only the plural `completion_column_ids`, which never shipped, so a real legacy client's write was still silently discarded. `POST /v1/boards` keeps ignoring the singular key as it always has (a newly created board has no columns, so nothing could be lost there). Set `default_status` on a column via the column endpoints (`POST /v1/boards/:board_id/columns`, `PATCH /v1/columns/:id`, `PUT /v1/boards/:board_id/columns/:id`) and read it back on `ColumnResponse.default_status`.
