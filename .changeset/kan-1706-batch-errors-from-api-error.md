---
bump: minor
---

remote batch failures are rebuilt through `From<ApiError>`: a server fault in a batch over HTTP surfaces as an internal error instead of a validation error. `RemoteBatchOutcome.failed` is now `Vec<(Uuid, ApiError)>`, and `kanban-backend` depends on `kanban-api`.
