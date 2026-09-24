---
bump: minor
---

domain, backend, backend-http, backend-memory, cli, persistence-json,
persistence-sqlite, service, tui: removes `snapshot`/`apply_snapshot` from
the `DataStore` trait's required surface and deletes the 24 impl pairs
across every backend and test double that satisfied it. Every whole-store
read/write already went through `store_adapter::read_full_snapshot` /
`write_full_snapshot`, which compose per-entity `DataStore` calls instead
of a single whole-store method, so this is a pure trait-surface reduction
with no caller to repair. `kanban_domain::Snapshot`, the type, is
unaffected and stays.
