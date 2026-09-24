---
bump: patch
---

mcp: retarget every sprint tool and `tool_export_board` off the `McpResolve` shim onto the call-scoped `Model` built via `ToolScope`/`ToolScoped`, delete the now-dead `McpResolve` trait and the unused `ToolScope::renders_board_entity` field, and fix a `ToolScope::next_round` gap where a named global sprint reference never requested the board list needed to resolve it.
