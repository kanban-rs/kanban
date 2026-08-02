---
bump: patch
---

Fixed a dependency-graph integrity bug: restoring one of two archived cards that
shared a dependency edge (spawns/blocks/relates) revived the edge even though the
other card was still archived, leaving an active dependency pointing at a hidden
card. Restoring a card now only revives edges whose other endpoint is also live,
matching the invariant already enforced when an edge is created.
