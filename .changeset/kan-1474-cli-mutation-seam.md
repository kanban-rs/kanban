---
bump: patch
---

cli: introduces `CliContext::mutate`/`mutate_unit`, the sole internal seam through which every mutating handler now consumes the `Invalidation` a mutation returns. No public API changes; all mutation call sites in `handlers/` and `app.rs` route through the new `pub(crate)` seam instead of the `KanbanOperations`/`GraphOperations` trait methods directly.
