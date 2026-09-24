---
bump: patch
---

domain: DeleteCard::touched_entities now names the card it deletes and the dependency graph instead of returning None. Every card create derives its invalidation from its captured DeleteCard inverse, so creates stop broadening to Invalidation::All and name the created card; the prefix-counter bump they also perform is still declared through execute_with_extra. Card deletes are unaffected on the forward path (their invalidation comes from the ImportEntities inverse) but a redo of a delete is now scoped too.
