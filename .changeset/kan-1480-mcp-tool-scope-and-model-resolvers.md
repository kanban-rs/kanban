---
bump: minor
---

mcp: adds `McpContext::model_for`, `sync_into` and `sync_invalidated`, three new public methods for building and refreshing a call-scoped `Model` over the shared `KanbanContext::sync`/`sync_invalidated` seam. No tool body changes; leaves the seven `mcp_resolve_*` shims and every tool untouched.
