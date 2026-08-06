---
bump: minor
---

view: extract renderer-agnostic `kanban-view` crate from `kanban-tui`'s view-model layer (`Model`, `LayoutStrategy`/`ViewStrategy`, `CardList`/`ListComponent`, filter/search state, selection-dialog and sprint-assign-list logic, scroll-indicator and panel-title formatting), so a future `kanban-web` frontend can reuse it instead of re-deriving it from scratch. `kanban-view`'s dependency set is locked to `kanban-core`/`kanban-domain`/`serde`/`uuid`/`chrono` (no rendering framework), enforced in CI.
