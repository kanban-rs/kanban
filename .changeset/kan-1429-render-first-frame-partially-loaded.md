---
bump: minor
---

tui: `App::load_initial_state` now runs two scoped `populate` passes over the board list and the auto-selected board's subtree instead of one whole-store `snapshot` read, and `check_ended_sprints` declines on a `NotLoaded` sprint tier instead of scanning it through the collapsing `Model::sprints()` accessor.
