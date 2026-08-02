---
bump: patch
---

`kanban-server` now accepts the data file path as a positional argument (`kanban-server <path>`), matching `kanban-cli` and `kanban-mcp`. Previously it only read the `KANBAN_FILE` environment variable, falling back to the config file's `storage_location`. Precedence is unchanged for existing setups: an explicit positional argument now wins over `KANBAN_FILE`, which still wins over the config-file default.
