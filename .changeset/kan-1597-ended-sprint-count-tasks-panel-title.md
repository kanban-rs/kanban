---
bump: minor
---

view: adds ended_sprint_count(...) -> PanelCount and a TasksPanelTitle::ended_sprints field, so a title can say how many of a board's sprints ran past their end date without closing out, or say it does not know. build_tasks_panel_title gains a matching parameter. This is a minor bump: it adds a public field to a non-exhaustive-free struct and a parameter to a public fn, breaking every out-of-tree caller.

tui: the main-view tasks panel title now reports the active board's ended sprints, styled with the new theme::ended_marker() that ui/board_detail.rs's " Ended" marker also consumes. PanelConfig carries a ratatui Line instead of a &str so the segment can be styled, and format_tasks_panel_title keeps its String signature as that Line's Display projection.
