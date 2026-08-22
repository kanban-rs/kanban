---
bump: patch
---

service: `KanbanContext` gains an opt-in per-entity read cache. Surfaces call `with_entity_cache()` to enable it and `resolve(plan)` to fetch only what a plan still needs; committed command batches, undos and redos now drop the entities they touched from that cache. Contexts that do not opt in behave exactly as before.
