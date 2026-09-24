---
bump: patch
---

server: route every mutation site through a single `mutate`/`mutate_unit` seam in `state.rs` that owns the `Invalidation` a mutation produces, instead of calling `KanbanOperations` trait methods or raw inherent `*_impl` methods directly from the routes and handlers.
