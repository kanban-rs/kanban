---
bump: patch
---

domain: `Model::invalidate` now narrows a `cards` invalidation to just the affected `cards_by_column`/`scoped_card_index` scopes when `EntityIds::card_columns` names every touched card's columns, falling back to today's whole-tier drop otherwise. The four `archived_*` tiers still drop on every card invalidation. No producer populates `card_columns` yet, so every call still takes the fallback path and behaviour is unchanged.
