---
bump: minor
---

kanban-view, kanban-tui: `Controller::displayed_cards`/`displayed_boards`/`live_cards`/`archived_cards`/`archived_boards_view` now return `LoadState<&[Card]>`/`LoadState<&[Board]>` instead of a bare slice or iterator, and `App::displayed_cards`/`displayed_boards` mirror that as `LoadState<&[Card]>`/`LoadState<Vec<Board>>`. A freshly defaulted or unsynced `Controller` now reports its partitions as `NotLoaded` instead of silently collapsing them to empty. Every `kanban-tui` call site collapses explicitly at the render boundary with `.loaded().copied().unwrap_or(&[])` (or `.loaded().into_iter().flatten()` where an iterator is needed), a temporary marker for KAN-1431 to replace with an explicit not-loaded/failed render. `App::prepare_frame` declines to rebuild the task lists when the card, column, or sprint tier is not `Loaded`, matching the existing columns/sprints decline rule.
