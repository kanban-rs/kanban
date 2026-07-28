---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). `kanban-server` now resolves its data file the same way `kanban-cli` and `kanban-mcp` already do: it reads the shared config file (`~/.config/kanban/config.toml`, or `KANBAN_CONFIG` override), falling back to the same default filename (`boards.json`) instead of a `kanban-server`-specific one. `KANBAN_FILE` still overrides everything, same as before.

Also improves the error message when the data file can't be opened or parsed: it now names the exact file path that failed, and shows a clean message instead of a raw internal error representation.
