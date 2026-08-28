---
bump: patch
---

domain: add `DependencyGraph::filtered_to` and `DependencyGraph::merge_from`, the two whole-graph primitives that scoping an export and merging an import will build on. `filtered_to` keeps only edges whose endpoints are both in a given card set, preserving each edge's live/archived state, kind and metadata (`blocks` severity, `relates` kind). `merge_from` adds another graph's edges without disturbing existing ones, treating an edge already present as a no-op and rejecting a cycle-inducing edge as an error. Purely additive; no existing call site changes.
