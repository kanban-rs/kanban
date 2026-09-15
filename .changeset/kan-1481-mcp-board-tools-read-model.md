---
bump: patch
---

mcp: retarget board.rs's four name-resolution call sites (tool_get_board,
tool_update_board, tool_delete_board, tool_archive_board) from the
McpResolve::mcp_resolve_board shim to a call-scoped Model built through a new
ToolScope per request. A failed or unfetched board-list read now surfaces an
error naming "board list" instead of collapsing into a raw backend error.
