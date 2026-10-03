---
bump: patch
---

server: the four nested sprint lifecycle routes (`activate`, `complete`, `cancel`, `carry-over`) now wrap their response body in `MutationResponse<T>`, carrying the mutation's `Invalidation` alongside the entity fields. Non-breaking: the flattened body still deserializes as the bare `SprintResponse`/`CarryOverResponse`. The four handlers share one write-lock/persist/broadcast helper instead of four near-identical blocks.
