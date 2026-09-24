---
bump: minor
---

service: adds `kanban_service::resolve(plan, loaded, store) -> Resolved`, a new public free function that runs `FetchPlan` rounds against a `DataStore` without owning any state between calls. It reads whole-collection, per-id and the three parent-scoped tiers, deduping within a call and composing the caller's `LoadedEntities` with the in-flight `Resolved` so a scope discovered mid-call resolves in the same call. `resolve` returns `Resolved` directly rather than a `Result`, since backend failures already surface per-entity as `LoadState::Failed`.
