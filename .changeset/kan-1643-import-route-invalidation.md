---
bump: patch
---

server: `POST /v1/import` now returns a `MutationResponse<BoardResponse>`, carrying the import's own `invalidation` alongside the board. The response still deserializes as a bare `BoardResponse` (the wrap is additive via `#[serde(flatten)]`), so existing clients are unaffected.
