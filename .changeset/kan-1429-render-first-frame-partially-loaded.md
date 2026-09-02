---
bump: minor
---

domain, tui: `App::load_initial_state` now runs two scoped `populate` passes over the board list and the auto-selected board's subtree instead of one whole-store `snapshot` read, and `check_ended_sprints` declines on a `NotLoaded` sprint tier instead of scanning it through the collapsing `Model::sprints()` accessor. Startup no longer loads archival markers at all; entering the archived-boards or archived-cards view instead triggers a `reload_model` snapshot on first entry, gated by the new `Model::archived_boards_absorbed` / `Model::archived_card_markers_absorbed` predicates so a later mutation-driven reload is not repeated.
