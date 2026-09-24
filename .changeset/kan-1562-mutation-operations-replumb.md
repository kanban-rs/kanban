---
bump: minor
---

cli: the `mutate`/`mutate_unit` seam closures are now typed against `MutationOperations` instead of `KanbanContext`, so a seam closure can no longer reach a context read or an invalidation-stripping `KanbanOperations` mutator.
mcp: the `mutate`/`mutate_unit` seam closures are now typed against `MutationOperations` instead of `KanbanContext`, same restriction as above.
server: the `mutate`/`mutate_unit` seam closures are now typed against `MutationOperations` instead of `KanbanContext`, same restriction as above; the return shape is unchanged.
tui: `TuiContext` implements `MutationOperations` directly (every method flushes through the existing save-coordinator path) in place of six hand-written inherent forwarders.

Behaviour is unchanged across all four applications.
