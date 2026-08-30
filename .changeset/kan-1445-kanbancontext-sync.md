---
bump: minor
---

service: `KanbanContext::sync` and `KanbanContext::sync_invalidated` are new inherent methods that run a `FetchPlan` against a `Model` and fold the resolved entities back in, resyncing the caller's `DerivedProjections` with the `ModelChanged` receipt. `sync_invalidated` applies its `Invalidation` before the plan is consulted, so a tier a mutation touched is refetched instead of being left `Loaded` and skipped. Every application now shares one resolve-application step and none of them needs a `&dyn DataStore`.
