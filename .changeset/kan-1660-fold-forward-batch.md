---
bump: minor
---

domain, service: `MoveCard` and `CreateCard` now name their column in `EntityIds::card_columns`, and a forward mutation's `Invalidation` folds the forward batch's column hints into the inverse's (`kanban_domain::invalidation_from_batch`), so a card move invalidates only its source and destination column scopes instead of every column.
