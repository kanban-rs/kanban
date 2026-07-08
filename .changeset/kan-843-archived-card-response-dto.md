---
bump: patch
---

The CLI `card list --archived` output and the MCP `list_archived_cards` tool now serialize a stable v1 `ArchivedCardResponse` DTO instead of the internal domain summary. The archived-card JSON now nests the richer card projection (carrying `description` and `card_number`, hiding internal `sprint_logs`) and surfaces the archived card's `board_id` as a first-class field, matching the v1 DTO direction used elsewhere. An exhaustive mapping means a future `ArchivedCard` field must be deliberately represented (or omitted) in the DTO.
