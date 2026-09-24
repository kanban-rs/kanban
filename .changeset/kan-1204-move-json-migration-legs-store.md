---
bump: minor
---

service: adds `StoreManager::make_backend_named` for resolving a
`KanbanBackend` by exact name, bypassing header/extension sniffing (a
migration destination never exists yet, so there is nothing to sniff).
`migrate_store`'s JSON-side source and destination now route through
`make_backend`/`make_backend_named` and a typed `Snapshot` via the
`store_adapter` module instead of `PersistenceStore`/`StoreSnapshot`;
FK repair moves with it as `store_adapter::repair_fks`, replacing the
old JSON-level `repair_snapshot_fks`/`fix_card_fks` pair.
