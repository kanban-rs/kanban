---
bump: minor
---

kanban-view, kanban-tui: `Controller::displayed_cards`/`displayed_boards` now return `LoadState<&[Card]>`/`LoadState<&[Board]>` instead of a bare slice, and `App::displayed_cards`/`displayed_boards` mirror that as `LoadState<&[Card]>`/`LoadState<Vec<Board>>`. A freshly defaulted or unsynced `Controller` now reports its partitions as `NotLoaded` instead of silently collapsing them to empty. `App::prepare_frame` declines to rebuild the task lists when the card, column, or sprint tier is not `Loaded`, matching the existing columns/sprints decline rule.
