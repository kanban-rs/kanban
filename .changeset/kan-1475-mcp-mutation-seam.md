---
bump: minor
---

mcp: every mutating tool handler now goes through a new `McpContext::mutate` / `mutate_unit` seam that absorbs the `Invalidation` a service `*_impl` call produces and hands the tool only its result, replacing the `mutating_op!` macro (deleted). `McpContext::create_board_from_spec` now returns `KanbanResult<(Board, Invalidation)>` instead of `KanbanResult<Board>`, mirroring the KAN-1499 precedent on the other `create_*_from_spec` forwarders.
