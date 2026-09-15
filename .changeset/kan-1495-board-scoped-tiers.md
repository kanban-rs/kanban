---
bump: patch
---

domain: `Model::load_from_snapshot` now populates the board-scoped columns
and sprints tiers by grouping the snapshot's flat rows by board id, instead
of leaving them cleared and empty after every load.
