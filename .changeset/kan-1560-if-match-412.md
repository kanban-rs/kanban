---
bump: minor
---

server,api: add RFC 9110 If-Match optimistic concurrency to the 18 entity PUT/PATCH/DELETE routes (boards, cards, columns, sprints, both board-scoped and flat). A stale (or otherwise non-matching) If-Match on a conditioned write now returns 412 with the new `ErrorCode::PreconditionFailed`, mapped from a new `etag::check_if_match` helper that does RFC 9110 strong comparison with `*` support. The precondition check runs inside the existing per-request write lock, before the mutation, and is skipped entirely when no If-Match header is present. An ETag taken from an entity's GET is accepted by If-Match on that entity's own write routes. CORS under an explicit-origin policy now allows the `If-Match`/`If-None-Match` request headers and exposes `ETag` on responses, so browser clients can participate in the protocol.
