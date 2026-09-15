---
bump: minor
---

mcp: every mutating tool handler now goes through a new `McpContext::mutate` / `mutate_unit` seam that hands the tool the `(value, Invalidation)` pair a service `*_impl` call produces, replacing the `mutating_op!` macro (deleted). `McpContext::create_board_from_spec`, `create_column_from_spec`, `create_card_from_spec`, and `create_sprint_from_spec` are removed; tool handlers now call the equivalent `KanbanContext` method directly through `mutate`.
