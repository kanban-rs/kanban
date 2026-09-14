---
bump: patch
---

cli, mcp: fix `board list` / `list_boards` unconditionally fetching archived-board markers before filtering, which made every board listing fail against the HTTP backend (a remote `kanban-server`) even for the default live-only view. Both now reuse the service layer's selector-aware gather, which only fetches archived markers when the selector actually needs them.
