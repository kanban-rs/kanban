---
bump: patch
---

api,server,backend-http: expose prefixes read-only over the REST API (`GET /v1/prefixes`, `GET /v1/prefixes/{name}`), backed by a new `PrefixResponse` DTO. `HttpBackend::list_prefixes`/`get_prefix` now call these routes instead of failing with `Unsupported`; `upsert_prefix` stays `Unsupported` since no create/rename/delete route exists.
