---
bump: patch
---

Added board and column HTTP request types to the kanban-service api module (`kanban_service::api`): `CreateBoardRequest`, `UpdateBoardRequest`, `CreateColumnRequest`, `UpdateColumnRequest`, and `ReorderColumnRequest`. These are the wire types for the board and column create/update endpoints. `UpdateBoardRequest` deliberately excludes server-managed fields (`active_sprint_id`, board `position`), and the update types use `FieldUpdate<T>` for three-state (set/clear/no-change) semantics.
