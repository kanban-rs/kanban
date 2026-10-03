---
bump: minor
---

batch failures on `/v1/cards/batch/*` carry an `ErrorCode` and a client-safe message; server faults in a batch are no longer echoed to clients. `BatchOperationFailure` gains an `api_error` field (not serialized).
