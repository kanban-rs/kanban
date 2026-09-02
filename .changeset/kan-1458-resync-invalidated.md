---
bump: minor
---

kanban-service adds `KanbanContext::resync_invalidated`, a mutate-then-read sibling of `sync_invalidated` that repairs the tiers a `Model` had already read before an `Invalidation` blanks them, then runs the caller's own `FetchPlan`, folding both passes through one `resync` call.
