---
bump: minor
---

tui, persistence: drop the whole-store write-back hydration from storage migration.

`handle_migration_complete` no longer probes the incoming backend by reading a
whole `Snapshot` and writing it back into `App`. The pre-swap readability
check is now a narrow `list_boards()` call, and the sort-field/order sync
that used to ride along in the write-back reads directly from the new
backend via `data_store().get_board(..)` instead. This removes the public
`kanban_tui::state::TuiSnapshot` trait and `crates/kanban-tui/src/state/snapshot.rs`
entirely; its serialization round-trip test moves to `kanban-persistence`,
where the functions it exercises already live.
