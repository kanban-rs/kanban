---
bump: patch
---

Pressing `V`, `t`, `T`, or `1` while viewing archived tasks no longer leaks
into live state. Previously these keys fell through to the same handlers
used by the live tasks panel: `V` opened the task-list-view picker and
changed the board's display mode, `t` silently toggled the shared active-
sprint filter, `T` opened the filter-options dialog, and `1` jumped focus
out to the projects panel — all while nominally still looking at the
archived list. All four are now no-ops from the archived-cards view, and
are no longer advertised in its footer help.
