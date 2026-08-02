---
bump: patch
---

The archived-boards view now always defaults to recency order (most recently archived first), independent of whatever sort preference is saved for the live boards list. Previously, changing the live list's default sort (for example to sort by name) also silently changed the archived view's default, since both shared the same saved preference. This is now consistent everywhere: at the service layer (MCP/CLI/API) and in the terminal UI. The archived-boards panel in the TUI is still independently sortable via its own `s`/`o` keys, but that choice is session-only and no longer bleeds into the live view's default or gets persisted.
