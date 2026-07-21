---
bump: minor
---

The board list is now sortable. You can order boards by position, name,
creation time, or archival recency, and persist a default so every surface
respects it.

- CLI: `kanban board list --sort <field> --order <dir>` sorts a single listing
  (`field` is one of `position`, `name`, `created_at`, `archived_at`; `dir` is
  `asc` or `desc`). `kanban board set-sort --sort <field> --order <dir>`
  persists a default in the app config; later `board list` calls without an
  explicit sort/order use it.
- MCP: `list_boards` gains `sort` and `order` parameters, and a new
  `set_board_sort` tool persists the default.
- TUI: the projects panel gets a sort field picker (`o`) and an order toggle
  (`s`) that work for both the live and archived board views, and the choice is
  persisted across sessions.

The archived-boards view defaults to recency (most recently archived first)
rather than position.
