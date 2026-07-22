---
bump: patch
---

Fixed the MCP `set_board_sort` tool reporting `null` for a dimension that was
omitted from the request but actually resolved and persisted. When you set only
the field (or only the order), the response now echoes the concrete value that
was saved, matching the CLI.
