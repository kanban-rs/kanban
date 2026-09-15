---
bump: patch
---

cli: `kanban-cli` becomes a Controller. A new `pub(crate) CommandScope` folds the parsed `Commands` value into a `kanban_service::FetchPlan`, `CliContext` gains a `kanban_domain::Model` plus the stored scope, and `dispatch_subcommand` syncs the Model once before dispatch. The `mutate`/`mutate_unit` seam now applies its returned `Invalidation` into the Model via `KanbanContext::sync_invalidated` instead of discarding it. `KanbanOperations::resolve_board_id` and `GraphOperations::list_children_of`/`list_parents_of` are retargeted to read from the Model's `board_list` and `graph` tiers, behind a shared `pub(crate)` `require_loaded` helper. No public API changes.
