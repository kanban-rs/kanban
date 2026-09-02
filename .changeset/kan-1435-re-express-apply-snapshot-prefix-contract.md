---
bump: minor
---

service,backend-memory,persistence-sqlite: re-express the prefix contract
suite over `write_full_snapshot` instead of `DataStore::apply_snapshot`.
`test_apply_snapshot_stores_prefix_rows_normalised` and
`test_apply_snapshot_collapses_two_spellings_of_one_namespace` are renamed to
`test_a_whole_store_write_stores_prefix_rows_normalised` and
`test_a_whole_store_write_collapses_two_spellings_of_one_namespace`.
`test_a_referenced_namespace_cannot_be_removed_on_every_backend` is removed
and its guarantee is split into a new per-backend `apply_snapshot` rejection
test in `kanban-backend-memory` and `kanban-persistence-sqlite`, plus two new
cross-backend contract functions pinning `write_full_snapshot`'s merge
semantics: it never removes a namespace and still rejects a card whose
namespace has no backing row.
