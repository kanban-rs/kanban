---
bump: patch
---

The `set_board_sort` MCP tool now reports the actual sort field and order it resolved and persisted, instead of echoing back exactly what the caller sent. Previously, omitting either `sort` or `order` (to leave that dimension unchanged) made the response report `null` for the omitted half, even though a concrete value was computed and saved. The response now always reflects the real, effective sort configuration, matching how the CLI's equivalent command already behaved.
