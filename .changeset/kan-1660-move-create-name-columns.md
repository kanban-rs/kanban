---
bump: patch
---

domain: `MoveCard` and `CreateCard` now populate `EntityIds::card_columns` with the destination/creation column, and `invalidation_from_batch(forward, inverse)` folds both the forward batch and its captured inverse so a card move invalidates both its source and destination column scopes. `invalidation_from_inverse` is unchanged behaviourally, now delegating to `invalidation_from_batch(&[], inverse)`. Additive only; no caller switches to the new function in this change.
