---
bump: minor
---

domain,service: `Model` gains `invalidate(Invalidation) -> ModelChanged`, dropping the flat, per-id and parent-scoped tiers named by an `Invalidation`. A card, column, board or sprint id drops that entity's whole affected parent-scoped tier (`cards_by_column` for a card id, `columns_by_board`/`sprints_by_board` for a board id) rather than guessing which single scope moved, since `EntityIds` names child ids, not the parent key a scoped tier is keyed on. `scoped_card_index` entries are cleared alongside every `cards_by_column` drop. `Invalidation::All` and an empty `EntityIds` both reset the whole `Model`. The snapshot-derived archival markers are left untouched by an ordinary invalidation. `kanban-service` gains a cross-backend contract test proving a card moved between columns reads correctly on in-memory, JSON and SQLite after `invalidate` runs.
