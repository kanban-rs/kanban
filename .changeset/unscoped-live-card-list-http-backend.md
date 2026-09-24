---
bump: patch
---

cli, mcp: fix unscoped card listing (`card list` without `--board`, e.g. `--sprint`-only queries) unconditionally fetching the workspace-global archived-card marker collection before filtering, breaking every such query against the HTTP backend (a remote `kanban-server`) even for the default live-only view. `list_cards_detailed` now skips that fetch when the selector is live-only, mirroring the board-listing fix.
