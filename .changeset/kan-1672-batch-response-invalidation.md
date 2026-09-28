---
bump: minor
---

api,server: `BatchOperationResponse` now carries the mutation's `invalidation` on the wire (required on `new`, defaulting to `all` when absent on deserialize), and every `/v1/cards/batch/*` route forwards the exact invalidation it computed instead of discarding it after the broadcast.
