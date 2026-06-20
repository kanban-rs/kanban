---
bump: patch
---

Added board and column HTTP request types to the kanban-service api module (`kanban_service::api`): `CreateBoardRequest`, `UpdateBoardRequest`, `CreateColumnRequest`, `UpdateColumnRequest`, and `ReorderColumnRequest`. These are the wire types for the board and column create/update endpoints. `UpdateBoardRequest` carries every client-editable board field (`name`, `description`, `sprint_prefix`, `card_prefix`, `task_sort_field`, `task_sort_order`, `sprint_duration_days`, `task_list_view`, `completion_column_id`) and deliberately excludes only the server-managed ones (`active_sprint_id`, board `position`). The update types use `FieldUpdate<T>` for three-state (set/clear/no-change) semantics.
