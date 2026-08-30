---
bump: minor
---

kanban-domain, kanban-view, kanban-tui: `Model::apply_resolved`, `Model::mark_failed` and `Model::load_from_snapshot` now return a `#[must_use]` `ModelChanged` receipt instead of `()`. A new one-method `DerivedProjections` trait consumes that receipt, with a `NoProjections` no-op implementor for callers with nothing to derive. `kanban-view`'s inherent `Controller::sync` is replaced by `impl DerivedProjections for Controller`, so forgetting to recompute derived state after a `Model` mutation is now a compile-time lint rather than a doc-comment plea.
