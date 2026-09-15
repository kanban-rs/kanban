---
bump: minor
---

domain: `Model` gains a per-id tier and a parent-scoped tier for boards, columns, cards and sprints, alongside the flat collections it already held. `apply_resolved` applies all three independently, `load_from_snapshot` clears the new tiers so a whole-store load supersedes every tier, and `mark_failed` marks the flat collection, the per-id entries and the parent scopes without removing any of them. New public accessors expose each tier's `LoadState` so a caller can tell a scope that was never read from one that resolved empty. A `scoped_card_index` maps a card id to the column scope holding it, which keeps the per-id lookup chain from scanning every scope.
