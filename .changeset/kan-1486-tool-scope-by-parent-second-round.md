---
bump: patch
---

mcp: `ToolScope` gains `resolved_board`, `wants_board_columns`, `wants_board_sprints` and a `for_board` builder so a second sync round can request `columns_by_board`/`sprints_by_board` once a board reference is resolved, fixing `resolve_column_in_board` and `resolve_sprint_in_board` so they actually resolve a name reference. No public API added; `scope` stays `pub(crate)`.
