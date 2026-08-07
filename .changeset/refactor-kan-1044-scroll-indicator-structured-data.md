---
bump: patch
---

view: `scroll_indicators` now returns structured `ScrollIndicator { count, direction }` instead of pre-formatted terminal strings, so a non-terminal frontend can render its own wording. `kanban-tui` owns the `"  2 Tasks above"` formatting; terminal output is unchanged.
