---
bump: minor
---

backend-http,tui: serve every whole-store list read over HTTP by fanning out over the per-board routes that already work. `list_all_cards`, `list_all_columns`, `list_all_sprints` and `list_archived_cards` no longer decline as `Unsupported`, so unscoped listings (`kanban card list` with no `--board`, `tool_list_cards` with no board) work against a remote `kanban-server` for the first time. The fan-out unions live and archived boards, since archiving a board only records a marker and its columns and cards stay in the flat collections; ordering matches the in-memory backend. No server or domain changes. The TUI sprint-log migration now recognises a declined read with `is_unsupported()` instead of matching the operation name as a literal string, so further decline-chain churn cannot resurface the startup error entry.
