---
bump: patch
---

api,server,backend-http: expose prefixes read-only over the REST API (`GET /v1/prefixes` for the list, `GET /v1/prefixes?name=<value>` for a single row), backed by a new `PrefixResponse` DTO with high-water-mark-named fields (`last_card_number`/`last_sprint_number`). `HttpBackend::list_prefixes`/`get_prefix` now call these routes instead of failing with `Unsupported`; `upsert_prefix` stays `Unsupported` since no create/rename/delete route exists. The lookup takes `name` as a query parameter rather than a path segment so a value containing `/`, `#`, `?` or a space round-trips correctly, and so the empty prefix (a valid namespace) stays addressable.
