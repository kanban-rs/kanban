---
bump: patch
---

server: the three graph POST routes (`attach_children`, `add_block`, `add_related`) now return `MutationResponse<CardGraphResponse>`, carrying the mutation's `invalidation` instead of discarding it. Responses still deserialize as a bare `CardGraphResponse` (the wrap is additive via `#[serde(flatten)]`), so existing clients are unaffected. Also adds `POST /v1/cards/{id}/children/detach`, which runs `detach_children_impl`'s single transaction so a batch detach removes every edge or none.
