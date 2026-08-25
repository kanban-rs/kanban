---
bump: patch
---

service: single-board export now scopes dependency-graph edges (spawns, blocks, relates) to the exported board's own live and archived cards instead of carrying the whole workspace graph, using the merged `DependencyGraph::filtered_to`. Dropped cross-board edges are logged at `warn` level. Full-workspace export is unaffected.
