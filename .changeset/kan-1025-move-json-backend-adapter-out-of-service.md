---
bump: patch
---

Internal groundwork with no user-visible effect: the JSON backend adapter now lives in `kanban-persistence-json`, alongside the store it wraps, instead of `kanban-service`. A new `JsonBackendFactory` reports its backend name (`"json"`) and constructs a `JsonDataStore` directly from a locator. `kanban-service` still constructs the JSON backend directly; behaviour, storage formats, and commands are unchanged.
