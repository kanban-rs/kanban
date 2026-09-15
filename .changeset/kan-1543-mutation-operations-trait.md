---
bump: minor
---

domain: adds a `MutationOperations` trait over the invalidation-carrying mutation surface (the 35 `*_impl` mutators, the eight spec-shaped create/create-or-replace entry points, and `execute`) plus the four `*CreateOutcome` types, moved here from kanban-service.
service: `KanbanContext` implements `MutationOperations` by pure delegation to its existing inherent methods; re-exports the outcome types so no importer changes. No behaviour change and no call site is re-plumbed onto the trait yet.
