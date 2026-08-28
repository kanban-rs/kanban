---
bump: patch
---

domain: import now merges the imported dependency graph into the destination workspace instead of replacing it, so importing any export no longer deletes every `spawns`/`blocks`/`relates` edge already in the destination. Undo of an import-driven graph merge (via `ImportEntities`'s own `capture_inverse`) removes exactly the edges the merge newly added, leaving the destination's pre-import graph intact.
