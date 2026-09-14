---
bump: minor
---

Completion-column feature completeness across every surface. MCP `tool_create_board` gains `with_default_columns`, seeding the TODO/Doing/Complete template with matching default statuses like the CLI and TUI already do. The TUI create-column dialog can now set a column's `default_status` at creation instead of requiring the separate `s` popup afterwards. The board-settings column list marks the primary completion column (the first status=done column by position, where done cards actually file) distinctly from other done columns. CLI `board create --with-default-columns` now creates the board and its three columns as one atomic command batch, so a mid-seed failure can no longer leave a partial board and undo reverses the whole action in one step. Internally, the status/completion invariant is now encoded once: `CardMoveResult` carries placement only and the service derives the status sync via `target_status_for_column_move`, which drops its unused board parameter.
