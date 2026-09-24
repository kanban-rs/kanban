---
bump: patch
---

mcp: column tools resolve board and column names through the call-scoped Model instead of the legacy backend-direct resolvers, matching the pattern already in place for board and card_batch tools. Behavior and JSON output are unchanged; a failed backend read during resolution now names the collection that could not be loaded instead of surfacing the raw backend error text.
