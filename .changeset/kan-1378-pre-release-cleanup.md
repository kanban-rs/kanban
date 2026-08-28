---
bump: patch
---

Pre-release cleanup for v0.9.0.

Fixes a path where a failed snapshot read during a storage-location swap could write an empty snapshot back into the store the user just switched to. Renames `validate_branch_prefix` to `validate_prefix_format`, which is what it actually validates. Collapses the duplicate-id checks in entity import into one helper and adds the column and sprint coverage that was missing. Removes twenty unused public functions and the unused `PersistenceEvent` enum. Points the CLAUDE.md persistence sections at the code that defines the format versions instead of restating them.
