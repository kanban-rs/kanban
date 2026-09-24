---
bump: minor
---

mcp: breaking change to the MCP contract. `tool_get_column`, `tool_update_column`, `tool_delete_column`, `tool_reorder_column`, and the board-less column-name arm of `tool_list_cards` now require a `board` when `column` is a name; resolution by column UUID is unaffected. Columns belong to a board and column names are not unique across boards, so a board-less name lookup was ambiguous by construction and failed outright against a remote backend. `resolve_column_global` is removed; every name lookup now routes through the existing board-scoped resolver, which already works remotely.
