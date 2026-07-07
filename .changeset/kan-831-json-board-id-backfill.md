---
bump: patch
---

Internal persistence change with no user-visible behaviour. The JSON backend
gains a V7->V8 format migration that backfills `board_id` onto historical
`archived_cards` entries that predate the first-class field, reconstructing the
value from each entry's `original_column_id` -> column -> board mapping. An
archived card whose original column no longer resolves keeps a nil board id
(unrecoverable, tolerated under the first-class model rather than failing the
load). The migration also defensively drops any live `cards` entry that shadows
an archived card by id, hardening the "archived cards are a discrete peer
collection" invariant. A `.v7.backup` is written before the destructive step
and removed on success, matching the existing chain policy. The board-scoping
consumer that reads the backfilled field is deferred to a later slice.
