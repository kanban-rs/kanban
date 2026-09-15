---
bump: patch
---

tui: `App::prepare_frame` now declines to rebuild the task lists when the columns or sprints tier is not loaded, instead of silently collapsing to an empty collection.
