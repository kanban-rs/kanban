---
bump: minor
---

kanban-service adds `InvalidationPlan`, a `FetchPlan` built from an `Invalidation` plus a pre-invalidation `LoadedState` snapshot. It re-requests exactly the tiers `Model::invalidate` is about to blank, restricted to tiers the model had already read, so a mutation only refetches what was actually visible before it ran.
