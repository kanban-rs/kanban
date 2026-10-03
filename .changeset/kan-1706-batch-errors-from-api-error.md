---
bump: minor
---

remote batch failures are rebuilt through `From<ApiError>`: a server fault in a batch over HTTP surfaces as an internal error instead of a validation error. A client error in an all-failed remote batch now carries its code, e.g. `validation error: NOT_FOUND: Card <id> not found`. `RemoteBatchOutcome.failed` is now `Vec<(Uuid, ApiError)>`, and `kanban-backend` depends on `kanban-api`.
