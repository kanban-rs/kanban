---
bump: patch
---

tui: the 9 `Model::columns()`/`Model::sprints()` reads in `column_handlers.rs` and `board_handlers.rs` now decline with an error banner on a `NotLoaded` tier instead of silently treating it as empty. `handle_delete_column_key`, `handle_move_column_up`, `handle_move_column_down`, `create_column`, and `delete_column` read the board-scoped columns tier; the internal `board_delete_counts` helper now returns `None` when its columns or sprints tier is unloaded, and its two callers (`handle_delete_board_key`, `handle_delete_archived_board_key`) decline in step.
