---
bump: minor
---

cli,mcp,tui: remove direct completion-column configuration surfaces (--completion-columns CLI flag, MCP completion_column_ids field, and the TUI board-creation seeding call). Completion is now configured only by giving a column a default_status of done: use 'column update <name> --default-status done' in place of 'board update <board> --completion-columns <name>'. A legacy completion_column_ids key sent to the MCP update_board tool is now silently ignored rather than rejected, so older agent scripts do not hard-fail.
