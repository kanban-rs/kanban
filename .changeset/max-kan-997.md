---
bump: patch
---

Updating a column (rename, position change, WIP limit) now rejects a negative position, matching the rule already enforced when creating a column. This closes a gap where reordering a column through the MCP server accepted a negative position with no validation at all.
