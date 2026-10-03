---
bump: patch
---

domain: `Model::invalidate` now narrows a `cards` invalidation to just the affected `cards_by_column`/`scoped_card_index` scopes when `EntityIds::card_columns` names every touched card's columns, falling back to today's whole-tier drop otherwise. The four `archived_*` tiers still drop on every card invalidation. Takes effect in this release through KAN-1660, which makes card moves, creates and column-setting updates name their columns.
