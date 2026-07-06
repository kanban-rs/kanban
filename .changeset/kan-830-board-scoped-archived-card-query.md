---
bump: patch
---

Internal domain change with no user-visible behaviour. Adds an additive `DataStore::list_archived_cards_by_board` method (a functional default filtering the archived list by the `board_id` field first-class on `ArchivedCard`, with an in-memory override) that SQL backends will override with a single `WHERE board_id = ?` query once the persistence `board_id` column and its backfill land. Nothing consumes the method yet: the consumer-facing switch (repointing board-scoped archived listing and relaxing the column-delete guard) is deferred to land together with that backfill, so no query changes behaviour until `board_id` is actually populated. Also routes the archived-card map key through `ArchivedEntity::entity_id()` (no behaviour change).
