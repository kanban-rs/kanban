---
bump: patch
---

`kanban-mcp` now honors the `KANBAN_FILE` environment variable when no data-file argument is given, matching `kanban-cli` and `kanban-server`. Its startup errors now also name the data file they failed to open. If you already export `KANBAN_FILE` globally and start `kanban-mcp` with no argument, it will switch from the config/`boards.json` fallback to that env var's target.
