---
bump: patch
---

tui: populate_sprint_task_lists, copy_card_output, and get_current_sprint_selection_index now read the columns/sprints tiers through the state-preserving accessors instead of the collapsing `Model::columns()`/`Model::sprints()`, so a `NotLoaded` tier is declined with an error banner (where the call site can signal one) rather than silently treated as empty.
